//! Formatos de pixel codificados no tipo.
//!
//! Cada formato é um wrapper `#[repr(transparent)]` sobre um array de
//! subpixels, então uma fatia `&[Rgb8]` tem exatamente o layout de memória
//! de `&[u8]` com três valores por pixel. As conversões entre as duas visões
//! são verificadas pela crate `bytemuck` e não copiam dados.

use std::fmt::Debug;

use bytemuck::{Pod, Zeroable};

use crate::{Error, Result};

mod sealed {
    pub trait Sealed {}
    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for f32 {}
}

/// Tipo numérico de um canal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleType {
    /// Inteiro de 8 bits sem sinal, faixa `0..=255`.
    U8,
    /// Inteiro de 16 bits sem sinal, faixa `0..=65535`.
    U16,
    /// Ponto flutuante de 32 bits, faixa nominal `0.0..=1.0`.
    F32,
}

/// Valor de um canal. Implementado apenas para `u8`, `u16` e `f32`.
pub trait Subpixel:
    sealed::Sealed + Copy + Default + PartialEq + PartialOrd + Debug + Send + Sync + Pod
{
    /// Valor do branco nominal: `255`, `65535` ou `1.0`.
    const MAX: Self;
    /// Tipo numérico correspondente.
    const SAMPLE: SampleType;
}

impl Subpixel for u8 {
    const MAX: Self = u8::MAX;
    const SAMPLE: SampleType = SampleType::U8;
}

impl Subpixel for u16 {
    const MAX: Self = u16::MAX;
    const SAMPLE: SampleType = SampleType::U16;
}

impl Subpixel for f32 {
    const MAX: Self = 1.0;
    const SAMPLE: SampleType = SampleType::F32;
}

/// Formato de pixel em runtime, para caminhos em que o tipo não é conhecido
/// em tempo de compilação (decodificação de arquivos, captura de câmera).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PixelFormat {
    /// Luminância, 8 bits.
    L8,
    /// Luminância + alpha, 8 bits por canal.
    La8,
    /// RGB, 8 bits por canal.
    Rgb8,
    /// RGBA, 8 bits por canal.
    Rgba8,
    /// BGR, 8 bits por canal.
    Bgr8,
    /// Luminância, 16 bits.
    L16,
    /// Luminância, `f32`.
    LF32,
    /// RGB, `f32` por canal.
    RgbF32,
}

impl PixelFormat {
    /// Número de canais por pixel.
    #[must_use]
    pub const fn channels(self) -> usize {
        match self {
            Self::L8 | Self::L16 | Self::LF32 => 1,
            Self::La8 => 2,
            Self::Rgb8 | Self::Bgr8 | Self::RgbF32 => 3,
            Self::Rgba8 => 4,
        }
    }

    /// Tipo numérico de cada canal.
    #[must_use]
    pub const fn sample(self) -> SampleType {
        match self {
            Self::L8 | Self::La8 | Self::Rgb8 | Self::Rgba8 | Self::Bgr8 => SampleType::U8,
            Self::L16 => SampleType::U16,
            Self::LF32 | Self::RgbF32 => SampleType::F32,
        }
    }

    /// Tamanho de um pixel em bytes.
    #[must_use]
    pub const fn bytes_per_pixel(self) -> usize {
        let sample = match self.sample() {
            SampleType::U8 => 1,
            SampleType::U16 => 2,
            SampleType::F32 => 4,
        };
        self.channels() * sample
    }
}

/// Um pixel: grupo de [`Pixel::CHANNELS`] subpixels contíguos em memória.
pub trait Pixel: Copy + Default + PartialEq + Debug + Send + Sync + Pod {
    /// Tipo de cada canal.
    type Subpixel: Subpixel;
    /// Número de canais.
    const CHANNELS: usize;
    /// Formato correspondente em runtime.
    const FORMAT: PixelFormat;

    /// Canais na ordem em que ficam na memória.
    fn channels(&self) -> &[Self::Subpixel];

    /// Canais mutáveis na ordem em que ficam na memória.
    fn channels_mut(&mut self) -> &mut [Self::Subpixel];

    /// Reinterpreta uma fatia de subpixels como pixels, sem cópia.
    ///
    /// # Errors
    /// [`Error::BufferLength`] se o comprimento não for múltiplo de
    /// [`Pixel::CHANNELS`].
    fn cast_slice(subpixels: &[Self::Subpixel]) -> Result<&[Self]> {
        bytemuck::try_cast_slice(subpixels).map_err(|_| Error::BufferLength {
            len: subpixels.len(),
            channels: Self::CHANNELS,
        })
    }

    /// Versão mutável de [`Pixel::cast_slice`].
    ///
    /// # Errors
    /// [`Error::BufferLength`] se o comprimento não for múltiplo de
    /// [`Pixel::CHANNELS`].
    fn cast_slice_mut(subpixels: &mut [Self::Subpixel]) -> Result<&mut [Self]> {
        let len = subpixels.len();
        bytemuck::try_cast_slice_mut(subpixels).map_err(|_| Error::BufferLength {
            len,
            channels: Self::CHANNELS,
        })
    }

    /// Reinterpreta uma fatia de pixels como subpixels, sem cópia.
    #[must_use]
    fn as_subpixels(pixels: &[Self]) -> &[Self::Subpixel] {
        // Nunca falha: mesmo alinhamento e tamanho múltiplo do subpixel.
        bytemuck::cast_slice(pixels)
    }

    /// Versão mutável de [`Pixel::as_subpixels`].
    #[must_use]
    fn as_subpixels_mut(pixels: &mut [Self]) -> &mut [Self::Subpixel] {
        bytemuck::cast_slice_mut(pixels)
    }
}

macro_rules! pixel_format {
    ($(#[$doc:meta])* $name:ident, $sub:ty, $channels:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
        #[repr(transparent)]
        pub struct $name(pub [$sub; $channels]);

        impl Pixel for $name {
            type Subpixel = $sub;
            const CHANNELS: usize = $channels;
            const FORMAT: PixelFormat = PixelFormat::$name;

            fn channels(&self) -> &[$sub] {
                &self.0
            }

            fn channels_mut(&mut self) -> &mut [$sub] {
                &mut self.0
            }
        }
    };
}

macro_rules! luma_accessors {
    ($name:ident, $sub:ty) => {
        impl $name {
            /// Cria um pixel a partir da luminância.
            #[must_use]
            pub const fn new(luma: $sub) -> Self {
                Self([luma])
            }

            /// Luminância.
            #[must_use]
            pub const fn luma(self) -> $sub {
                self.0[0]
            }
        }
    };
}

macro_rules! rgb_accessors {
    ($name:ident, $sub:ty, $r:literal, $g:literal, $b:literal) => {
        impl $name {
            /// Cria um pixel a partir de vermelho, verde e azul.
            #[must_use]
            pub const fn new(r: $sub, g: $sub, b: $sub) -> Self {
                let mut px = [r; 3];
                px[$g] = g;
                px[$b] = b;
                px[$r] = r;
                Self(px)
            }

            /// Vermelho.
            #[must_use]
            pub const fn r(self) -> $sub {
                self.0[$r]
            }

            /// Verde.
            #[must_use]
            pub const fn g(self) -> $sub {
                self.0[$g]
            }

            /// Azul.
            #[must_use]
            pub const fn b(self) -> $sub {
                self.0[$b]
            }
        }
    };
}

pixel_format!(
    /// Luminância de 8 bits.
    L8, u8, 1
);
pixel_format!(
    /// Luminância + alpha, 8 bits por canal. Memória: `[L, A]`.
    La8, u8, 2
);
pixel_format!(
    /// RGB de 8 bits por canal. Memória: `[R, G, B]`.
    Rgb8, u8, 3
);
pixel_format!(
    /// RGBA de 8 bits por canal. Memória: `[R, G, B, A]`.
    Rgba8, u8, 4
);
pixel_format!(
    /// BGR de 8 bits por canal. Memória: `[B, G, R]`.
    Bgr8, u8, 3
);
pixel_format!(
    /// Luminância de 16 bits.
    L16, u16, 1
);
pixel_format!(
    /// Luminância em `f32`, faixa nominal `0.0..=1.0`.
    LF32, f32, 1
);
pixel_format!(
    /// RGB em `f32` por canal, faixa nominal `0.0..=1.0`. Memória: `[R, G, B]`.
    RgbF32, f32, 3
);

luma_accessors!(L8, u8);
luma_accessors!(L16, u16);
luma_accessors!(LF32, f32);
rgb_accessors!(Rgb8, u8, 0, 1, 2);
rgb_accessors!(Bgr8, u8, 2, 1, 0);
rgb_accessors!(RgbF32, f32, 0, 1, 2);

impl La8 {
    /// Cria um pixel a partir de luminância e alpha.
    #[must_use]
    pub const fn new(luma: u8, alpha: u8) -> Self {
        Self([luma, alpha])
    }

    /// Luminância.
    #[must_use]
    pub const fn luma(self) -> u8 {
        self.0[0]
    }

    /// Alpha.
    #[must_use]
    pub const fn a(self) -> u8 {
        self.0[1]
    }
}

impl Rgba8 {
    /// Cria um pixel a partir de vermelho, verde, azul e alpha.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    /// Vermelho.
    #[must_use]
    pub const fn r(self) -> u8 {
        self.0[0]
    }

    /// Verde.
    #[must_use]
    pub const fn g(self) -> u8 {
        self.0[1]
    }

    /// Azul.
    #[must_use]
    pub const fn b(self) -> u8 {
        self.0[2]
    }

    /// Alpha.
    #[must_use]
    pub const fn a(self) -> u8 {
        self.0[3]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;
    use std::mem::{align_of, size_of};

    fn assert_layout<P: Pixel>() {
        assert_eq!(size_of::<P>(), P::CHANNELS * size_of::<P::Subpixel>());
        assert_eq!(align_of::<P>(), align_of::<P::Subpixel>());
        assert_eq!(P::FORMAT.channels(), P::CHANNELS);
        assert_eq!(P::FORMAT.bytes_per_pixel(), size_of::<P>());
    }

    #[test]
    fn every_format_has_the_layout_of_its_subpixel_array() {
        assert_layout::<L8>();
        assert_layout::<La8>();
        assert_layout::<Rgb8>();
        assert_layout::<Rgba8>();
        assert_layout::<Bgr8>();
        assert_layout::<L16>();
        assert_layout::<LF32>();
        assert_layout::<RgbF32>();
    }

    #[test]
    fn format_constants_match_the_type() {
        assert_eq!(Rgb8::FORMAT, PixelFormat::Rgb8);
        assert_eq!(Bgr8::FORMAT, PixelFormat::Bgr8);
        assert_eq!(L16::FORMAT, PixelFormat::L16);
        assert_eq!(RgbF32::FORMAT, PixelFormat::RgbF32);
        assert_eq!(PixelFormat::Rgba8.channels(), 4);
        assert_eq!(PixelFormat::RgbF32.bytes_per_pixel(), 12);
        assert_eq!(PixelFormat::L16.sample(), SampleType::U16);
    }

    #[test]
    fn subpixel_max_is_nominal_white() {
        assert_eq!(<u8 as Subpixel>::MAX, 255);
        assert_eq!(<u16 as Subpixel>::MAX, 65535);
        assert!((<f32 as Subpixel>::MAX - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn named_accessors_follow_memory_order() {
        let rgb = Rgb8::new(1, 2, 3);
        assert_eq!((rgb.r(), rgb.g(), rgb.b()), (1, 2, 3));
        assert_eq!(rgb.channels(), &[1, 2, 3]);

        // BGR guarda o azul primeiro na memória.
        let bgr = Bgr8::new(1, 2, 3);
        assert_eq!((bgr.r(), bgr.g(), bgr.b()), (1, 2, 3));
        assert_eq!(bgr.channels(), &[3, 2, 1]);

        let rgba = Rgba8::new(1, 2, 3, 4);
        assert_eq!(rgba.a(), 4);
        assert_eq!(La8::new(9, 8).channels(), &[9, 8]);
        assert_eq!(L8::new(7).luma(), 7);
    }

    #[test]
    fn channels_mut_writes_through() {
        let mut px = Rgb8::default();
        px.channels_mut()[1] = 200;
        assert_eq!(px, Rgb8::new(0, 200, 0));
    }

    #[test]
    fn subpixel_slice_casts_to_pixels_without_copy() {
        let raw = [1u8, 2, 3, 4, 5, 6];
        let pixels = Rgb8::cast_slice(&raw).unwrap();
        assert_eq!(pixels, &[Rgb8::new(1, 2, 3), Rgb8::new(4, 5, 6)]);
        assert_eq!(pixels.as_ptr().cast::<u8>(), raw.as_ptr());
        assert_eq!(Rgb8::as_subpixels(pixels), &raw);
    }

    #[test]
    fn cast_rejects_length_that_is_not_a_multiple_of_channels() {
        let raw = [0u8; 7];
        assert!(matches!(
            Rgb8::cast_slice(&raw),
            Err(Error::BufferLength {
                len: 7,
                channels: 3
            })
        ));
    }

    #[test]
    fn mutable_cast_writes_through() {
        let mut raw = [0u16; 4];
        L16::cast_slice_mut(&mut raw).unwrap()[2] = L16::new(500);
        assert_eq!(raw, [0, 0, 500, 0]);

        let mut pixels = [RgbF32::default(); 2];
        RgbF32::as_subpixels_mut(&mut pixels)[4] = 0.5;
        assert_eq!(pixels[1], RgbF32::new(0.0, 0.5, 0.0));
    }

    #[test]
    fn empty_slice_casts_to_empty() {
        assert_eq!(Rgba8::cast_slice(&[]).unwrap(), &[]);
    }
}
