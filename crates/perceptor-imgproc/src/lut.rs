//! Tabelas de consulta (LUT): uma função de um canal, pré-calculada.
//!
//! Qualquer transformação que dependa **só do valor** do canal — gamma,
//! brilho/contraste, inversão, curva de tons — se reduz a `out = tabela[in]`.
//! Calcular a tabela custa 256 (ou 65 536) avaliações, uma vez; aplicar custa
//! um acesso à memória por amostra, qualquer que seja a função.
//!
//! [`Lut<u8>`] serve aos formatos de 8 bits (256 entradas) e [`Lut<u16>`] a
//! `L16` (65 536). Formatos `f32` não têm tabela: não há um número finito de
//! níveis para indexar.
//!
//! | Construtor | `out` (com `m` = nível máximo) |
//! |---|---|
//! | [`identity`](Lut::identity) | `v` |
//! | [`invert`](Lut::invert) | `m − v` |
//! | [`gamma`](Lut::gamma) | `m · (v / m)^γ` |
//! | [`gain_offset`](Lut::gain_offset) | `α · v + β` |
//! | [`from_fn`](Lut::from_fn) | qualquer função de `u8`/`u16` |
//!
//! Gamma e ganho/offset arredondam para o inteiro mais próximo (metade para
//! longe de zero) e **saturam** em `[0, m]`. Parâmetro `NaN` produz `0`.

use perceptor_core::{Error, Image, ImageView, ImageViewMut, Pixel, Result};

/// Amostra inteira que uma [`Lut`] sabe indexar: `u8` ou `u16`.
pub trait LutSample: Copy + private::Sealed {
    /// Número de níveis distintos: `256` ou `65 536`.
    const LEVELS: usize;
    /// Nível máximo como `f64`.
    const MAX_F64: f64;
    /// Posição da amostra na tabela.
    fn index(self) -> usize;
    /// Valor da amostra como `f64`, sem perda.
    fn to_f64(self) -> f64;
    /// Amostra de um nível. `level < LEVELS`.
    fn from_level(level: usize) -> Self;
    /// Arredonda e satura em `[0, MAX]`. `NaN` vira `0`.
    fn saturate(value: f64) -> Self;
}

mod private {
    pub trait Sealed {}
    impl Sealed for u8 {}
    impl Sealed for u16 {}
}

// `as` de `f64` para inteiro já satura nos extremos e leva `NaN` a 0.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
impl LutSample for u8 {
    const LEVELS: usize = 256;
    const MAX_F64: f64 = 255.0;
    fn index(self) -> usize {
        usize::from(self)
    }
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_level(level: usize) -> Self {
        level as u8
    }
    fn saturate(value: f64) -> Self {
        value.round() as u8
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
impl LutSample for u16 {
    const LEVELS: usize = 65_536;
    const MAX_F64: f64 = 65_535.0;
    fn index(self) -> usize {
        usize::from(self)
    }
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_level(level: usize) -> Self {
        level as u16
    }
    fn saturate(value: f64) -> Self {
        value.round() as u16
    }
}

/// Tabela de consulta com uma entrada por nível da amostra `T`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lut<T: LutSample> {
    table: Box<[T]>,
}

impl<T: LutSample> Lut<T> {
    /// Tabela de uma função arbitrária, avaliada uma vez por nível.
    #[must_use]
    pub fn from_fn(mut f: impl FnMut(T) -> T) -> Self {
        let table = (0..T::LEVELS)
            .map(|level| f(T::from_level(level)))
            .collect();
        Self { table }
    }

    /// `out = v`.
    #[must_use]
    pub fn identity() -> Self {
        Self::from_fn(|v| v)
    }

    /// `out = m − v`: negativo da imagem.
    #[must_use]
    pub fn invert() -> Self {
        let max = T::LEVELS - 1;
        Self::from_fn(|v| T::from_level(max - v.index()))
    }

    /// `out = m · (v / m)^γ`, com `m` o nível máximo.
    ///
    /// `γ < 1` clareia os tons médios, `γ > 1` os escurece; `0` e `m` ficam
    /// onde estão. É a potência direta, sem inverter o expoente: para
    /// *codificar* para exibição com um gamma de tela `g`, use `γ = 1 / g`.
    #[must_use]
    pub fn gamma(gamma: f64) -> Self {
        Self::from_fn(|v| T::saturate(T::MAX_F64 * (v.to_f64() / T::MAX_F64).powf(gamma)))
    }

    /// `out = α · v + β`, saturado em `[0, m]`.
    ///
    /// `gain` (α) muda o contraste e `offset` (β) o brilho. `α < 0` inverte.
    #[must_use]
    pub fn gain_offset(gain: f64, offset: f64) -> Self {
        Self::from_fn(|v| T::saturate(gain * v.to_f64() + offset))
    }

    /// As entradas da tabela, indexadas pelo nível de entrada.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.table
    }

    #[inline]
    fn get(&self, sample: T) -> T {
        // `index() < LEVELS == table.len()` por construção.
        self.table[sample.index()]
    }
}

/// Aplica a mesma tabela a **todos** os canais de cada pixel.
///
/// Em formatos com alfa (`La8`, `Rgba8`) o alfa também passa pela tabela; use
/// [`apply_lut_per_channel`] com uma identidade nele para preservá-lo.
///
/// # Errors
/// [`Error::Allocation`] se a saída não puder ser alocada.
pub fn apply_lut<P>(input: ImageView<'_, P>, lut: &Lut<P::Subpixel>) -> Result<Image<P>>
where
    P: Pixel,
    P::Subpixel: LutSample,
{
    let mut output = Image::<P>::new(input.width(), input.height())?;
    for (dst, src) in output.rows_mut().zip(input.rows()) {
        for (out, pixel) in dst.iter_mut().zip(src) {
            *out = *pixel;
            for channel in out.channels_mut() {
                *channel = lut.get(*channel);
            }
        }
    }
    Ok(output)
}

/// Aplica a mesma tabela a todos os canais, no lugar, sem alocar.
pub fn apply_lut_in_place<P>(mut image: ImageViewMut<'_, P>, lut: &Lut<P::Subpixel>)
where
    P: Pixel,
    P::Subpixel: LutSample,
{
    for pixel in image.pixels_mut() {
        for channel in pixel.channels_mut() {
            *channel = lut.get(*channel);
        }
    }
}

/// Aplica uma tabela a cada canal: `luts[i]` transforma o canal `i`.
///
/// # Errors
/// - [`Error::ChannelMismatch`] se `luts.len()` não for o número de canais
///   do formato.
/// - [`Error::Allocation`] se a saída não puder ser alocada.
pub fn apply_lut_per_channel<P>(
    input: ImageView<'_, P>,
    luts: &[Lut<P::Subpixel>],
) -> Result<Image<P>>
where
    P: Pixel,
    P::Subpixel: LutSample,
{
    if luts.len() != P::CHANNELS {
        return Err(Error::ChannelMismatch {
            expected: P::CHANNELS,
            actual: luts.len(),
        });
    }
    let mut output = Image::<P>::new(input.width(), input.height())?;
    for (dst, src) in output.rows_mut().zip(input.rows()) {
        for (out, pixel) in dst.iter_mut().zip(src) {
            *out = *pixel;
            for (channel, lut) in out.channels_mut().iter_mut().zip(luts) {
                *channel = lut.get(*channel);
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturate_clamps_rounds_and_maps_nan_to_zero() {
        assert_eq!(<u8 as LutSample>::saturate(-5.0), 0);
        assert_eq!(<u8 as LutSample>::saturate(300.0), 255);
        assert_eq!(<u8 as LutSample>::saturate(2.5), 3);
        assert_eq!(<u8 as LutSample>::saturate(f64::NAN), 0);
        assert_eq!(<u16 as LutSample>::saturate(70_000.0), 65_535);
    }

    #[test]
    fn invert_is_an_involution() {
        let table = Lut::<u8>::invert();
        assert!((0..=255u8).all(|v| table.get(table.get(v)) == v));
    }
}
