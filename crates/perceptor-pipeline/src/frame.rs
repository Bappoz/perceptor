//! Componente `Frame`: unidade fundamental de dados no pipeline.
//!
//! Um [`Frame`] associa metadados a uma imagem em qualquer formato de pixel.
//! O formato é conhecido em runtime ([`Frame::format`]) e o acesso aos pixels
//! é tipado: [`Frame::as_image`] devolve `&Image<P>` ou um erro se `P` não
//! for o formato dos dados.

use std::sync::Arc;
use std::time::Duration;

use bevy_ecs::prelude::Component;
use image::DynamicImage;
use perceptor_core::{
    Bgr8, Error, Image, La8, Pixel, PixelFormat, Result, Rgb8, RgbF32, Rgba8, L16, L8, LF32,
};

/// Metadados de um frame de vídeo/imagem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameMeta {
    /// Índice sequencial no stream (0-based).
    pub index: u64,
    /// Instante de apresentação, medido desde o início do stream.
    pub timestamp: Duration,
    /// Fonte de origem (nome do arquivo, device ID, URL…).
    ///
    /// `Arc<str>`: todos os frames de uma fonte compartilham a mesma string,
    /// e clonar os metadados não aloca.
    pub source: Arc<str>,
}

impl FrameMeta {
    /// Metadados com timestamp zero.
    pub fn new(index: u64, source: impl Into<Arc<str>>) -> Self {
        Self {
            index,
            timestamp: Duration::ZERO,
            source: source.into(),
        }
    }

    /// Define o instante de apresentação.
    #[must_use]
    pub fn with_timestamp(mut self, timestamp: Duration) -> Self {
        self.timestamp = timestamp;
        self
    }
}

/// Pixels de um [`Frame`]: uma [`Image`] em um dos formatos suportados.
///
/// Construa com `FrameData::from(imagem)`. O enum é `#[non_exhaustive]`:
/// novos formatos podem surgir, então um `match` externo precisa de um
/// braço `_`. Na maior parte dos casos [`FrameData::as_image`] é mais direto.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum FrameData {
    /// Luminância, 8 bits.
    L8(Image<L8>),
    /// Luminância + alpha, 8 bits por canal.
    La8(Image<La8>),
    /// RGB, 8 bits por canal.
    Rgb8(Image<Rgb8>),
    /// RGBA, 8 bits por canal.
    Rgba8(Image<Rgba8>),
    /// BGR, 8 bits por canal.
    Bgr8(Image<Bgr8>),
    /// Luminância, 16 bits.
    L16(Image<L16>),
    /// Luminância, `f32`.
    LF32(Image<LF32>),
    /// RGB, `f32` por canal.
    RgbF32(Image<RgbF32>),
}

/// Aplica a mesma expressão à imagem de qualquer variante.
macro_rules! for_any_format {
    ($data:expr, $image:ident => $body:expr) => {
        match $data {
            FrameData::L8($image) => $body,
            FrameData::La8($image) => $body,
            FrameData::Rgb8($image) => $body,
            FrameData::Rgba8($image) => $body,
            FrameData::Bgr8($image) => $body,
            FrameData::L16($image) => $body,
            FrameData::LF32($image) => $body,
            FrameData::RgbF32($image) => $body,
        }
    };
}

impl FrameData {
    /// Formato de pixel dos dados.
    #[must_use]
    pub fn format(&self) -> PixelFormat {
        match self {
            Self::L8(_) => PixelFormat::L8,
            Self::La8(_) => PixelFormat::La8,
            Self::Rgb8(_) => PixelFormat::Rgb8,
            Self::Rgba8(_) => PixelFormat::Rgba8,
            Self::Bgr8(_) => PixelFormat::Bgr8,
            Self::L16(_) => PixelFormat::L16,
            Self::LF32(_) => PixelFormat::LF32,
            Self::RgbF32(_) => PixelFormat::RgbF32,
        }
    }

    /// Largura em pixels.
    #[must_use]
    pub fn width(&self) -> usize {
        for_any_format!(self, image => image.width())
    }

    /// Altura em pixels.
    #[must_use]
    pub fn height(&self) -> usize {
        for_any_format!(self, image => image.height())
    }

    /// O buffer como bytes, em ordem de linha e na ordem de bytes nativa.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        for_any_format!(self, image => image.as_bytes())
    }

    /// A imagem, se o formato dos dados for `P`.
    ///
    /// # Errors
    /// [`Error::FormatMismatch`] se os dados estiverem em outro formato.
    pub fn as_image<P: FramePixel>(&self) -> Result<&Image<P>> {
        let actual = self.format();
        P::get(self).ok_or(Error::FormatMismatch {
            expected: P::FORMAT,
            actual,
        })
    }

    /// A imagem, mutável, se o formato dos dados for `P`.
    ///
    /// # Errors
    /// [`Error::FormatMismatch`] se os dados estiverem em outro formato.
    pub fn as_image_mut<P: FramePixel>(&mut self) -> Result<&mut Image<P>> {
        let actual = self.format();
        P::get_mut(self).ok_or(Error::FormatMismatch {
            expected: P::FORMAT,
            actual,
        })
    }

    /// Extrai a imagem sem copiar, se o formato dos dados for `P`.
    ///
    /// # Errors
    /// Devolve os próprios dados, intactos, se estiverem em outro formato.
    pub fn into_image<P: FramePixel>(self) -> std::result::Result<Image<P>, Self> {
        P::take(self)
    }
}

mod sealed {
    pub trait Sealed {}
}

/// Formato de pixel que um [`Frame`] pode carregar.
///
/// Implementado para todos os formatos de `perceptor-core`; não pode ser
/// implementado fora desta crate.
pub trait FramePixel: Pixel + sealed::Sealed {
    #[doc(hidden)]
    fn get(data: &FrameData) -> Option<&Image<Self>>;
    #[doc(hidden)]
    fn get_mut(data: &mut FrameData) -> Option<&mut Image<Self>>;
    #[doc(hidden)]
    fn take(data: FrameData) -> std::result::Result<Image<Self>, FrameData>;
}

macro_rules! frame_pixel {
    ($($format:ident),* $(,)?) => {
        $(
            impl sealed::Sealed for $format {}

            impl FramePixel for $format {
                fn get(data: &FrameData) -> Option<&Image<Self>> {
                    match data {
                        FrameData::$format(image) => Some(image),
                        _ => None,
                    }
                }

                fn get_mut(data: &mut FrameData) -> Option<&mut Image<Self>> {
                    match data {
                        FrameData::$format(image) => Some(image),
                        _ => None,
                    }
                }

                fn take(data: FrameData) -> std::result::Result<Image<Self>, FrameData> {
                    match data {
                        FrameData::$format(image) => Ok(image),
                        other => Err(other),
                    }
                }
            }

            impl From<Image<$format>> for FrameData {
                fn from(image: Image<$format>) -> Self {
                    Self::$format(image)
                }
            }
        )*
    };
}

frame_pixel!(L8, La8, Rgb8, Rgba8, Bgr8, L16, LF32, RgbF32);

/// Componente principal: uma imagem com metadados.
///
/// `data` é público: um sistema que muda o formato do frame (ex.: RGB →
/// escala de cinza) substitui o campo inteiro.
#[derive(Component, Debug, Clone)]
pub struct Frame {
    /// Metadados do frame.
    pub meta: FrameMeta,
    /// Pixels, em um dos formatos de [`FrameData`].
    pub data: FrameData,
}

impl Frame {
    /// Cria um frame a partir de uma imagem em qualquer formato suportado.
    pub fn new(meta: FrameMeta, data: impl Into<FrameData>) -> Self {
        Self {
            meta,
            data: data.into(),
        }
    }

    /// Converte um [`DynamicImage`] da crate `image` em [`Frame`] RGB8.
    ///
    /// Para preservar o canal alpha, use [`Frame::from_dynamic_image_rgba`].
    ///
    /// # Errors
    /// [`Error::InvalidDimensions`] para imagem vazia, ou [`Error::Allocation`].
    pub fn from_dynamic_image(meta: FrameMeta, img: DynamicImage) -> Result<Self> {
        let image = Image::<Rgb8>::from_image_buffer(&img.into_rgb8())?;
        Ok(Self::new(meta, image))
    }

    /// Converte um [`DynamicImage`] preservando canal alpha (RGBA8).
    ///
    /// # Errors
    /// [`Error::InvalidDimensions`] para imagem vazia, ou [`Error::Allocation`].
    pub fn from_dynamic_image_rgba(meta: FrameMeta, img: DynamicImage) -> Result<Self> {
        let image = Image::<Rgba8>::from_image_buffer(&img.into_rgba8())?;
        Ok(Self::new(meta, image))
    }

    /// A imagem, se o formato do frame for `P`.
    ///
    /// # Errors
    /// [`Error::FormatMismatch`] se o frame estiver em outro formato.
    pub fn as_image<P: FramePixel>(&self) -> Result<&Image<P>> {
        self.data.as_image()
    }

    /// A imagem, mutável, se o formato do frame for `P`.
    ///
    /// # Errors
    /// [`Error::FormatMismatch`] se o frame estiver em outro formato.
    pub fn as_image_mut<P: FramePixel>(&mut self) -> Result<&mut Image<P>> {
        self.data.as_image_mut()
    }

    /// Formato de pixel dos dados.
    #[must_use]
    pub fn format(&self) -> PixelFormat {
        self.data.format()
    }

    /// Altura em pixels.
    #[must_use]
    pub fn height(&self) -> usize {
        self.data.height()
    }

    /// Largura em pixels.
    #[must_use]
    pub fn width(&self) -> usize {
        self.data.width()
    }

    /// Número de canais por pixel.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.format().channels()
    }

    /// Retorna `true` se o frame tem um único canal (`L8`, `L16` ou `LF32`).
    #[must_use]
    pub fn is_grayscale(&self) -> bool {
        self.channels() == 1
    }
}
