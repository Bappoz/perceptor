//! Interoperabilidade com a crate `image`.
//!
//! `image::ImageBuffer` guarda subpixels intercalados em ordem de linha, o
//! mesmo layout das visões do Perceptor, então emprestar um como o outro não
//! copia nada. As conversões que produzem uma [`Image`] copiam, porque o
//! `Vec` de um `ImageBuffer` não tem o alinhamento de 64 bytes.

use std::ops::Deref;

use image::ImageBuffer;

use crate::{Error, Image, ImageView, La8, Pixel, Result, Rgb8, RgbF32, Rgba8, L16, L8};

/// Formato de pixel com equivalente direto na crate `image`.
pub trait ImagePixel: Pixel {
    /// Tipo de pixel correspondente em `image`.
    type Image: image::Pixel<Subpixel = Self::Subpixel> + 'static;
}

macro_rules! image_pixel {
    ($($ours:ty => $theirs:ty),* $(,)?) => {
        $(impl ImagePixel for $ours {
            type Image = $theirs;
        })*
    };
}

image_pixel! {
    L8 => image::Luma<u8>,
    La8 => image::LumaA<u8>,
    Rgb8 => image::Rgb<u8>,
    Rgba8 => image::Rgba<u8>,
    L16 => image::Luma<u16>,
    RgbF32 => image::Rgb<f32>,
}

impl<'a, P: ImagePixel> ImageView<'a, P> {
    /// Visão sem cópia sobre os pixels de um `ImageBuffer`.
    ///
    /// # Errors
    /// [`Error::InvalidDimensions`] se o buffer tiver largura ou altura zero.
    pub fn from_image_buffer<C>(buffer: &'a ImageBuffer<P::Image, C>) -> Result<Self>
    where
        C: Deref<Target = [P::Subpixel]>,
    {
        let (width, height) = buffer.dimensions();
        // `u32` sempre cabe em `usize` nas plataformas suportadas.
        let width = usize::try_from(width).unwrap_or(usize::MAX);
        let height = usize::try_from(height).unwrap_or(usize::MAX);
        Self::new(
            buffer.as_raw(),
            width,
            height,
            width.saturating_mul(P::CHANNELS),
        )
    }

    /// Empresta a visão como `ImageBuffer`, sem cópia.
    ///
    /// Devolve `None` se as linhas tiverem preenchimento (um `ImageBuffer`
    /// não representa stride) ou se alguma dimensão não couber em `u32`.
    #[must_use]
    pub fn as_image_buffer(&self) -> Option<ImageBuffer<P::Image, &'a [P::Subpixel]>> {
        if !self.is_contiguous() {
            return None;
        }
        let width = u32::try_from(self.width()).ok()?;
        let height = u32::try_from(self.height()).ok()?;
        ImageBuffer::from_raw(width, height, self.data)
    }

    /// Copia a região para um `ImageBuffer` dono dos dados.
    ///
    /// # Errors
    /// [`Error::InvalidDimensions`] se alguma dimensão não couber em `u32`.
    pub fn to_image_buffer(&self) -> Result<ImageBuffer<P::Image, Vec<P::Subpixel>>> {
        let invalid = || Error::InvalidDimensions {
            width: self.width(),
            height: self.height(),
        };
        let width = u32::try_from(self.width()).map_err(|_| invalid())?;
        let height = u32::try_from(self.height()).map_err(|_| invalid())?;

        let mut raw = Vec::with_capacity(self.width() * self.height() * P::CHANNELS);
        for row in self.rows() {
            raw.extend_from_slice(P::as_subpixels(row));
        }
        ImageBuffer::from_raw(width, height, raw).ok_or_else(invalid)
    }
}

impl<P: ImagePixel> Image<P> {
    /// Copia um `ImageBuffer` para uma imagem alinhada.
    ///
    /// # Errors
    /// - [`Error::InvalidDimensions`] se o buffer tiver largura ou altura zero.
    /// - [`Error::Allocation`] se o alocador não atender ao pedido.
    pub fn from_image_buffer<C>(buffer: &ImageBuffer<P::Image, C>) -> Result<Self>
    where
        C: Deref<Target = [P::Subpixel]>,
    {
        ImageView::from_image_buffer(buffer)?.to_image()
    }

    /// Copia a imagem para um `ImageBuffer` dono dos dados.
    ///
    /// # Errors
    /// [`Error::InvalidDimensions`] se alguma dimensão não couber em `u32`.
    pub fn to_image_buffer(&self) -> Result<ImageBuffer<P::Image, Vec<P::Subpixel>>> {
        self.view().to_image_buffer()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Error, Image, ImageView, La8, Rect, Rgb8, RgbF32, Rgba8, L16, L8};
    use image::{GrayImage, ImageBuffer, Luma, Rgb, RgbImage};
    use proptest::prelude::*;

    #[test]
    fn view_over_image_buffer_is_zero_copy() {
        let buffer = RgbImage::from_fn(3, 2, |x, y| {
            Rgb([u8::try_from(x).unwrap(), u8::try_from(y).unwrap(), 9])
        });
        let view = ImageView::<Rgb8>::from_image_buffer(&buffer).unwrap();
        assert_eq!(view.dimensions(), (3, 2));
        assert_eq!(view.get(2, 1), Some(&Rgb8::new(2, 1, 9)));
        assert_eq!(view.row(0).as_ptr().cast::<u8>(), buffer.as_raw().as_ptr());
    }

    #[test]
    fn owned_conversions_round_trip_every_supported_format() {
        fn round_trip<P: super::ImagePixel>(image: &Image<P>) {
            let buffer = image.to_image_buffer().unwrap();
            assert_eq!(buffer.dimensions(), (3, 2));
            assert_eq!(&Image::<P>::from_image_buffer(&buffer).unwrap(), image);
        }
        round_trip(&Image::filled(3, 2, L8::new(7)).unwrap());
        round_trip(&Image::filled(3, 2, La8::new(7, 8)).unwrap());
        round_trip(&Image::filled(3, 2, Rgb8::new(1, 2, 3)).unwrap());
        round_trip(&Image::filled(3, 2, Rgba8::new(1, 2, 3, 4)).unwrap());
        round_trip(&Image::filled(3, 2, L16::new(40_000)).unwrap());
        round_trip(&Image::filled(3, 2, RgbF32::new(0.25, 0.5, 1.0)).unwrap());
    }

    #[test]
    fn from_image_buffer_copies_into_aligned_storage() {
        let buffer = GrayImage::from_raw(2, 2, vec![1, 2, 3, 4]).unwrap();
        let image = Image::<L8>::from_image_buffer(&buffer).unwrap();
        assert_eq!(image.as_subpixels(), &[1, 2, 3, 4]);
        assert!(image.as_bytes().as_ptr().addr().is_multiple_of(64));
    }

    #[test]
    fn contiguous_view_borrows_as_image_buffer_and_padded_view_does_not() {
        let image = Image::from_fn(4, 3, |x, y| L8::new(u8::try_from(y * 4 + x).unwrap())).unwrap();
        let borrowed = image.view().as_image_buffer().unwrap();
        assert_eq!(borrowed.get_pixel(3, 2), &Luma([11]));
        assert_eq!(borrowed.as_raw().as_ptr(), image.as_subpixels().as_ptr());

        let roi = image.roi(Rect::new(1, 1, 2, 2)).unwrap();
        assert!(roi.as_image_buffer().is_none());
        // Cópia compacta continua disponível para visões com preenchimento.
        let copied = roi.to_image_buffer().unwrap();
        assert_eq!(copied.as_raw(), &[5, 6, 9, 10]);
    }

    #[test]
    fn empty_image_buffer_is_rejected() {
        let empty: ImageBuffer<Luma<u8>, Vec<u8>> = ImageBuffer::new(0, 3);
        assert!(matches!(
            ImageView::<L8>::from_image_buffer(&empty),
            Err(Error::InvalidDimensions {
                width: 0,
                height: 3
            })
        ));
    }

    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)] // centenas de casos: lento demais sob interpretação
        fn rgb_round_trip_preserves_every_pixel(w in 1u32..16, h in 1u32..16, seed in any::<u8>()) {
            let buffer = RgbImage::from_fn(w, h, |x, y| {
                let v = seed.wrapping_add(u8::try_from((x * 31 + y * 17) % 256).unwrap());
                Rgb([v, v.wrapping_mul(3), v.wrapping_add(100)])
            });
            let image = Image::<Rgb8>::from_image_buffer(&buffer).unwrap();
            prop_assert_eq!(image.to_image_buffer().unwrap(), buffer);
        }
    }
}
