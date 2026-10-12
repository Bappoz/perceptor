//! Conversões de layout de canais: reordenar, acrescentar, remover, separar
//! e juntar.
//!
//! Nenhuma destas funções faz conta com os valores: elas só movem subpixels.
//! Por isso todo par conversão/inversa é exato, e as que não mudam o número
//! de canais têm versão no lugar ([`swap_channels`]).
//!
//! | Função | Entrada → saída |
//! |---|---|
//! | [`rgb_to_bgr`], [`bgr_to_rgb`] | `Rgb8` ↔ `Bgr8` |
//! | [`add_alpha`] | `Rgb8` → `Rgba8`, `L8` → `La8` |
//! | [`drop_alpha`] | `Rgba8` → `Rgb8`, `La8` → `L8` |
//! | [`bgr_to_rgba`], [`rgba_to_bgr`] | `Bgr8` ↔ `Rgba8` |
//! | [`swap_channels`] | qualquer formato, no lugar |
//! | [`extract_channel`], [`split_channels`] | multicanal → planos de um canal |
//! | [`merge_channels`] | planos de um canal → multicanal |
//!
//! Índices de canal seguem a **ordem em memória** do formato: em `Bgr8` o
//! canal `0` é o azul.

use crate::map_pixels;
use perceptor_core::{
    Bgr8, Error, Image, ImageView, ImageViewMut, La8, Pixel, Result, Rgb8, RgbF32, Rgba8, L8, LF32,
};

/// Formato sem alpha que tem um correspondente com alpha.
pub trait Opaque: Pixel {
    /// O mesmo formato com um canal alpha no fim.
    type WithAlpha: Alpha<Opaque = Self>;

    /// O pixel com o alpha dado.
    fn with_alpha(self, alpha: u8) -> Self::WithAlpha;
}

/// Formato cujo último canal é alpha.
pub trait Alpha: Pixel {
    /// O mesmo formato sem o canal alpha.
    type Opaque: Pixel;

    /// O pixel sem o alpha; a cor não é multiplicada por ele.
    fn opaque(self) -> Self::Opaque;
}

impl Opaque for Rgb8 {
    type WithAlpha = Rgba8;

    fn with_alpha(self, alpha: u8) -> Rgba8 {
        Rgba8::new(self.r(), self.g(), self.b(), alpha)
    }
}

impl Alpha for Rgba8 {
    type Opaque = Rgb8;

    fn opaque(self) -> Rgb8 {
        Rgb8::new(self.r(), self.g(), self.b())
    }
}

impl Opaque for L8 {
    type WithAlpha = La8;

    fn with_alpha(self, alpha: u8) -> La8 {
        La8::new(self.luma(), alpha)
    }
}

impl Alpha for La8 {
    type Opaque = L8;

    fn opaque(self) -> L8 {
        L8::new(self.luma())
    }
}

/// Formato multicanal que pode ser separado em planos de um canal.
pub trait Planar: Pixel {
    /// Formato de um canal com o mesmo tipo de subpixel.
    type Plane: Pixel<Subpixel = Self::Subpixel>;
}

macro_rules! planar {
    ($($format:ty => $plane:ty),* $(,)?) => {
        $(impl Planar for $format {
            type Plane = $plane;
        })*
    };
}

planar!(La8 => L8, Rgb8 => L8, Bgr8 => L8, Rgba8 => L8, RgbF32 => LF32);

fn check_channel<P: Pixel>(index: usize) -> Result<()> {
    if index < P::CHANNELS {
        Ok(())
    } else {
        Err(Error::ChannelOutOfRange {
            index,
            channels: P::CHANNELS,
        })
    }
}

/// Converte `Rgb8` em `Bgr8`: a cor de cada pixel é a mesma, a ordem dos
/// bytes se inverte.
///
/// # Errors
/// [`Error::Allocation`] se a saída não puder ser alocada.
pub fn rgb_to_bgr(input: ImageView<'_, Rgb8>) -> Result<Image<Bgr8>> {
    map_pixels(input, |p| Bgr8::new(p.r(), p.g(), p.b()))
}

/// Converte `Bgr8` em `Rgb8`. Inversa de [`rgb_to_bgr`].
///
/// # Errors
/// [`Error::Allocation`] se a saída não puder ser alocada.
pub fn bgr_to_rgb(input: ImageView<'_, Bgr8>) -> Result<Image<Rgb8>> {
    map_pixels(input, |p| Rgb8::new(p.r(), p.g(), p.b()))
}

/// Acrescenta um canal alpha constante: `Rgb8` → `Rgba8`, `L8` → `La8`.
///
/// # Errors
/// [`Error::Allocation`] se a saída não puder ser alocada.
pub fn add_alpha<P: Opaque>(input: ImageView<'_, P>, alpha: u8) -> Result<Image<P::WithAlpha>> {
    map_pixels(input, |p| p.with_alpha(alpha))
}

/// Remove o canal alpha: `Rgba8` → `Rgb8`, `La8` → `L8`.
///
/// O alpha é descartado, não aplicado: um pixel transparente mantém a cor
/// que tinha. Compor sobre um fundo é outra operação.
///
/// # Errors
/// [`Error::Allocation`] se a saída não puder ser alocada.
pub fn drop_alpha<P: Alpha>(input: ImageView<'_, P>) -> Result<Image<P::Opaque>> {
    map_pixels(input, Alpha::opaque)
}

/// Converte `Bgr8` em `Rgba8` com alpha constante, em uma passada.
///
/// # Errors
/// [`Error::Allocation`] se a saída não puder ser alocada.
pub fn bgr_to_rgba(input: ImageView<'_, Bgr8>, alpha: u8) -> Result<Image<Rgba8>> {
    map_pixels(input, |p| Rgba8::new(p.r(), p.g(), p.b(), alpha))
}

/// Converte `Rgba8` em `Bgr8`, descartando o alpha.
///
/// # Errors
/// [`Error::Allocation`] se a saída não puder ser alocada.
pub fn rgba_to_bgr(input: ImageView<'_, Rgba8>) -> Result<Image<Bgr8>> {
    map_pixels(input, |p| Bgr8::new(p.r(), p.g(), p.b()))
}

/// Troca os canais `a` e `b` de cada pixel, no lugar e sem alocar.
///
/// O tipo da visão não muda: depois de `swap_channels(rgb, 0, 2)` a memória
/// está em ordem BGR, mas continua tipada como `Rgb8`. Serve para corrigir
/// um buffer cuja ordem real não é a do tipo (um decodificador que entrega
/// BGR, por exemplo); para obter uma imagem tipada como `Bgr8`, use
/// [`rgb_to_bgr`].
///
/// # Errors
/// [`Error::ChannelOutOfRange`] se `a` ou `b` não for um canal do formato;
/// nesse caso a imagem não é alterada.
pub fn swap_channels<P: Pixel>(mut image: ImageViewMut<'_, P>, a: usize, b: usize) -> Result<()> {
    check_channel::<P>(a)?;
    check_channel::<P>(b)?;
    if a != b {
        for pixel in image.pixels_mut() {
            pixel.channels_mut().swap(a, b);
        }
    }
    Ok(())
}

fn plane_pixel<P: Planar>(pixel: &P, index: usize) -> P::Plane {
    let mut plane = P::Plane::default();
    plane.channels_mut()[0] = pixel.channels()[index];
    plane
}

/// Copia o canal `index` para uma imagem de um canal.
///
/// # Errors
/// - [`Error::ChannelOutOfRange`] se `index` não for um canal do formato.
/// - [`Error::Allocation`] se a saída não puder ser alocada.
pub fn extract_channel<P: Planar>(
    input: ImageView<'_, P>,
    index: usize,
) -> Result<Image<P::Plane>> {
    check_channel::<P>(index)?;
    map_pixels(input, |p| plane_pixel(&p, index))
}

/// Separa a imagem em um plano por canal, na ordem em memória.
///
/// # Errors
/// [`Error::Allocation`] se algum plano não puder ser alocado.
pub fn split_channels<P: Planar>(input: ImageView<'_, P>) -> Result<Vec<Image<P::Plane>>> {
    (0..P::CHANNELS)
        .map(|index| map_pixels(input, |p| plane_pixel(&p, index)))
        .collect()
}

/// Junta planos de um canal em uma imagem multicanal: `planes[i]` vira o
/// canal `i`, na ordem em memória do formato de saída. Inversa de
/// [`split_channels`].
///
/// # Errors
/// - [`Error::ChannelMismatch`] se o número de planos não for o de canais do
///   formato de saída.
/// - [`Error::ShapeMismatch`] se algum plano tiver dimensões diferentes do
///   primeiro.
/// - [`Error::Allocation`] se a saída não puder ser alocada.
pub fn merge_channels<P: Planar>(planes: &[ImageView<'_, P::Plane>]) -> Result<Image<P>> {
    let Some(first) = planes.first().filter(|_| planes.len() == P::CHANNELS) else {
        return Err(Error::ChannelMismatch {
            expected: P::CHANNELS,
            actual: planes.len(),
        });
    };
    let expected = (first.width(), first.height());
    if let Some(other) = planes.iter().find(|p| (p.width(), p.height()) != expected) {
        return Err(Error::ShapeMismatch {
            expected,
            actual: (other.width(), other.height()),
        });
    }

    let mut output = Image::<P>::new(expected.0, expected.1)?;
    for (index, plane) in planes.iter().enumerate() {
        for (dst, src) in output.rows_mut().zip(plane.rows()) {
            for (out, value) in dst.iter_mut().zip(src) {
                out.channels_mut()[index] = value.channels()[0];
            }
        }
    }
    Ok(output)
}
