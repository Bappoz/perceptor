//! Conversão de profundidade (`u8` ↔ `u16` ↔ `f32`) e normalização.
//!
//! É a ponte entre a imagem decodificada (bytes) e o tensor que um modelo
//! espera (`f32`, geralmente em `[0, 1]` e centrado). Duas famílias:
//!
//! | Função | Faz | Quando |
//! |---|---|---|
//! | [`convert_scale`] | `saída = α · v + β` sobre o valor **numérico** | ajuste livre, como `convertTo` do `OpenCV` |
//! | [`convert_depth`] | mapeia a faixa **nominal** (`255`, `65535`, `1.0`) | trocar de profundidade sem mudar o brilho |
//! | [`normalize_minmax`] | estica `[mín, máx]` observado para `[lo, hi]` | contraste, entrada em `[0, 1]` |
//! | [`normalize_mean_std`] | `(v − média) / desvio` por canal | entrada de redes (`ImageNet` etc.) |
//! | [`channel_mean_std`] | média e desvio de cada canal | padronizar pela própria imagem |
//!
//! A origem e o destino precisam ter o **mesmo número de canais** (`L8` →
//! `LF32`, `Rgb8` → `RgbF32`); o valor de cada canal é convertido por conta
//! própria, sem tocar na ordem dos canais.
//!
//! # Arredondamento e saturação
//!
//! Destino inteiro: arredonda para o mais próximo (metade para longe de zero)
//! e **satura** em `[0, máx]`; `NaN` vira `0`. Destino `f32`: sem corte.
//! Nenhuma conversão estoura — os `as` de `f64` para inteiro saturam.
//!
//! # Tons nominais
//!
//! As funções de normalização trabalham em *tons nominais*: o valor dividido
//! pelo branco do formato de origem (`v / 255`, `v / 65535`, ou o próprio
//! `f32`). Assim, `mean = 0.5` significa "cinza médio" em qualquer
//! profundidade, e os valores usuais das redes (`0.485`, `0.229`…) valem
//! sem conta.

use perceptor_core::{Error, Image, ImageView, Pixel, Result};

mod sealed {
    pub trait Sealed {}
    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for f32 {}
}

/// Amostra numérica de um canal: `u8`, `u16` ou `f32`.
pub trait Sample: Copy + sealed::Sealed {
    /// Branco nominal como `f64`: `255`, `65535` ou `1.0`.
    const NOMINAL_MAX: f64;
    /// Valor numérico, sem perda.
    fn to_f64(self) -> f64;
    /// Volta ao tipo: arredonda e satura nos inteiros, estreita em `f32`.
    fn from_f64(value: f64) -> Self;
}

// `as` de `f64` para inteiro satura nos extremos e leva `NaN` a 0.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
impl Sample for u8 {
    const NOMINAL_MAX: f64 = 255.0;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_f64(value: f64) -> Self {
        value.round() as u8
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
impl Sample for u16 {
    const NOMINAL_MAX: f64 = 65_535.0;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_f64(value: f64) -> Self {
        value.round() as u16
    }
}

impl Sample for f32 {
    const NOMINAL_MAX: f64 = 1.0;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    #[allow(clippy::cast_possible_truncation)]
    fn from_f64(value: f64) -> Self {
        value as f32
    }
}

/// Média e desvio padrão populacional de um canal, em tons nominais.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelStats {
    /// Média dos tons nominais do canal.
    pub mean: f64,
    /// Desvio padrão populacional (divide por `N`, não por `N − 1`).
    pub std: f64,
}

fn check_channels<P: Pixel, Q: Pixel>() -> Result<()> {
    if P::CHANNELS == Q::CHANNELS {
        Ok(())
    } else {
        Err(Error::ChannelMismatch {
            expected: Q::CHANNELS,
            actual: P::CHANNELS,
        })
    }
}

/// Converte cada canal com `f`, para uma imagem nova de outro formato.
fn map_channels<P, Q>(input: ImageView<'_, P>, f: impl Fn(usize, f64) -> f64) -> Result<Image<Q>>
where
    P: Pixel,
    Q: Pixel,
    P::Subpixel: Sample,
    Q::Subpixel: Sample,
{
    check_channels::<P, Q>()?;
    let mut output = Image::<Q>::new(input.width(), input.height())?;
    for (dst, src) in output.rows_mut().zip(input.rows()) {
        for (out, pixel) in dst.iter_mut().zip(src) {
            let channels = pixel.channels().iter().zip(out.channels_mut());
            for (index, (value, slot)) in channels.enumerate() {
                *slot = Q::Subpixel::from_f64(f(index, value.to_f64()));
            }
        }
    }
    Ok(output)
}

/// `saída = α · v + β` sobre o valor numérico de cada canal, arredondado e
/// saturado no tipo de destino.
///
/// Não interpreta faixas: `u8` `100` com `α = 1` vira `f32` `100.0`. Para
/// preservar o brilho entre profundidades, use [`convert_depth`].
///
/// # Errors
/// - [`Error::ChannelMismatch`] se origem e destino tiverem número de canais
///   diferente.
/// - [`Error::Allocation`] se a saída não puder ser alocada.
pub fn convert_scale<P, Q>(input: ImageView<'_, P>, alpha: f64, beta: f64) -> Result<Image<Q>>
where
    P: Pixel,
    Q: Pixel,
    P::Subpixel: Sample,
    Q::Subpixel: Sample,
{
    map_channels(input, |_, v| alpha * v + beta)
}

/// Troca de profundidade preservando o brilho: o branco de uma vira o branco
/// da outra (`255` ↔ `65535` ↔ `1.0`).
///
/// `u8` → `u16` multiplica por 257 e volta exatamente; `u8` → `f32` → `u8`
/// também é exato para os 256 níveis.
///
/// # Errors
/// Os mesmos de [`convert_scale`].
pub fn convert_depth<P, Q>(input: ImageView<'_, P>) -> Result<Image<Q>>
where
    P: Pixel,
    Q: Pixel,
    P::Subpixel: Sample,
    Q::Subpixel: Sample,
{
    let scale = <Q::Subpixel as Sample>::NOMINAL_MAX / <P::Subpixel as Sample>::NOMINAL_MAX;
    map_channels(input, |_, v| v * scale)
}

/// Média e desvio de cada canal, em tons nominais.
///
/// Dois passes (média, depois variância) para não perder precisão em
/// imagens grandes.
#[must_use]
pub fn channel_mean_std<P>(input: ImageView<'_, P>) -> Vec<ChannelStats>
where
    P: Pixel,
    P::Subpixel: Sample,
{
    let white = <P::Subpixel as Sample>::NOMINAL_MAX;
    let mut sums = vec![0.0f64; P::CHANNELS];
    let mut count = 0u64;
    for pixel in input.pixels() {
        count += 1;
        for (sum, value) in sums.iter_mut().zip(pixel.channels()) {
            *sum += value.to_f64() / white;
        }
    }
    #[allow(clippy::cast_precision_loss)] // contagem de pixels, muito abaixo de 2^53
    let n = count as f64;
    let means: Vec<f64> = sums.iter().map(|s| s / n).collect();

    let mut squares = vec![0.0f64; P::CHANNELS];
    for pixel in input.pixels() {
        for ((sq, value), mean) in squares.iter_mut().zip(pixel.channels()).zip(&means) {
            let d = value.to_f64() / white - mean;
            *sq += d * d;
        }
    }
    means
        .into_iter()
        .zip(squares)
        .map(|(mean, sq)| ChannelStats {
            mean,
            std: (sq / n).sqrt(),
        })
        .collect()
}

/// Estica o intervalo observado `[mín, máx]` (de **todos** os canais juntos)
/// para `[lo, hi]`.
///
/// Usar o intervalo global preserva o equilíbrio entre os canais; esticar
/// cada canal sozinho deslocaria as cores. Imagem sem variação (`máx = mín`)
/// vira toda `lo`.
///
/// # Errors
/// - [`Error::InvalidParameter`] se `lo` ou `hi` não for finito.
/// - [`Error::ChannelMismatch`] se origem e destino tiverem número de canais
///   diferente.
/// - [`Error::Allocation`] se a saída não puder ser alocada.
pub fn normalize_minmax<P, Q>(input: ImageView<'_, P>, lo: f32, hi: f32) -> Result<Image<Q>>
where
    P: Pixel,
    Q: Pixel<Subpixel = f32>,
    P::Subpixel: Sample,
{
    if !lo.is_finite() || !hi.is_finite() {
        return Err(Error::InvalidParameter {
            name: "range",
            reason: "os limites de destino precisam ser finitos",
        });
    }
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    for pixel in input.pixels() {
        for value in pixel.channels() {
            let v = value.to_f64();
            min = min.min(v);
            max = max.max(v);
        }
    }
    let (lo, hi) = (f64::from(lo), f64::from(hi));
    // `!(max > min)` também cobre entradas só com `NaN`.
    let scale = if max > min {
        (hi - lo) / (max - min)
    } else {
        0.0
    };
    map_channels(input, |_, v| {
        if max > min {
            lo + (v - min) * scale
        } else {
            lo
        }
    })
}

/// `(v − média) / desvio` por canal, em tons nominais.
///
/// `mean` e `std` têm um valor por canal. Com `mean`/`std` de
/// [`channel_mean_std`], padroniza pela própria imagem; com os da base de
/// treino (`[0.485, 0.456, 0.406]` e `[0.229, 0.224, 0.225]` na `ImageNet`),
/// reproduz o pré-processamento do modelo.
///
/// # Errors
/// - [`Error::ChannelMismatch`] se `mean` ou `std` não tiverem um valor por
///   canal, ou se origem e destino tiverem número de canais diferente.
/// - [`Error::InvalidParameter`] se algum `std` não for finito e positivo, ou
///   algum `mean` não for finito.
/// - [`Error::Allocation`] se a saída não puder ser alocada.
pub fn normalize_mean_std<P, Q>(
    input: ImageView<'_, P>,
    mean: &[f32],
    std: &[f32],
) -> Result<Image<Q>>
where
    P: Pixel,
    Q: Pixel<Subpixel = f32>,
    P::Subpixel: Sample,
{
    for given in [mean.len(), std.len()] {
        if given != P::CHANNELS {
            return Err(Error::ChannelMismatch {
                expected: P::CHANNELS,
                actual: given,
            });
        }
    }
    if mean.iter().any(|m| !m.is_finite()) {
        return Err(Error::InvalidParameter {
            name: "mean",
            reason: "precisa ser finito",
        });
    }
    if std.iter().any(|s| !s.is_finite() || *s <= 0.0) {
        return Err(Error::InvalidParameter {
            name: "std",
            reason: "precisa ser finito e positivo",
        });
    }
    let white = <P::Subpixel as Sample>::NOMINAL_MAX;
    map_channels(input, |c, v| {
        (v / white - f64::from(mean[c])) / f64::from(std[c])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_f64_rounds_saturates_and_maps_nan_to_zero() {
        assert_eq!(u8::from_f64(-3.0), 0);
        assert_eq!(u8::from_f64(1e30), 255);
        assert_eq!(u8::from_f64(0.5), 1);
        assert_eq!(u8::from_f64(f64::NAN), 0);
        assert_eq!(u16::from_f64(70_000.0), 65_535);
        assert!((f32::from_f64(0.1) - 0.1f32).abs() < f32::EPSILON);
    }
}
