//! Interoperabilidade com a crate `ndarray`.
//!
//! A convenção de eixos é `[altura, largura, canais]`.

use ndarray::{Array3, ArrayView3, ShapeBuilder};

use crate::{Error, Image, ImageView, Pixel, Result};

fn check_channels<P: Pixel>(channels: usize) -> Result<()> {
    if channels == P::CHANNELS {
        Ok(())
    } else {
        Err(Error::ChannelMismatch {
            expected: P::CHANNELS,
            actual: channels,
        })
    }
}

impl<'a, P: Pixel> ImageView<'a, P> {
    /// Visão sem cópia sobre um array `[H, W, C]` em layout padrão (ordem de
    /// linha, contíguo).
    ///
    /// # Errors
    /// - [`Error::ChannelMismatch`] se o último eixo não tiver os canais de `P`.
    /// - [`Error::NonContiguous`] se o array não estiver em layout padrão
    ///   (fatiado, transposto); use [`Image::from_array3`] nesse caso.
    /// - [`Error::InvalidDimensions`] se altura ou largura forem zero.
    #[allow(clippy::needless_pass_by_value)] // `ArrayView` é `Copy`; por valor é a convenção do ndarray
    pub fn from_array3(array: ArrayView3<'a, P::Subpixel>) -> Result<Self> {
        let (height, width, channels) = array.dim();
        check_channels::<P>(channels)?;
        let data = array.to_slice().ok_or(Error::NonContiguous)?;
        Self::new(data, width, height, width * channels)
    }

    /// Empresta a visão como array `[H, W, C]`, sem cópia.
    ///
    /// Funciona também para visões com preenchimento: o stride vira o passo
    /// do primeiro eixo do array.
    ///
    /// # Panics
    /// Não ocorre: a geometria da visão já garante que todo elemento
    /// endereçado pelo array está dentro do buffer.
    #[must_use]
    pub fn as_array3(&self) -> ArrayView3<'a, P::Subpixel> {
        let shape = (self.height(), self.width(), P::CHANNELS);
        let strides = (self.stride(), P::CHANNELS, 1);
        ArrayView3::from_shape(shape.strides(strides), self.data)
            .expect("geometria validada na construção da visão")
    }
}

impl<P: Pixel> Image<P> {
    /// Copia um array `[H, W, C]` em qualquer layout para uma imagem alinhada.
    ///
    /// # Errors
    /// - [`Error::ChannelMismatch`] se o último eixo não tiver os canais de `P`.
    /// - [`Error::InvalidDimensions`] se altura ou largura forem zero.
    /// - [`Error::Allocation`] se o alocador não atender ao pedido.
    #[allow(clippy::needless_pass_by_value)] // `ArrayView` é `Copy`; por valor é a convenção do ndarray
    pub fn from_array3(array: ArrayView3<'_, P::Subpixel>) -> Result<Self> {
        let (height, width, channels) = array.dim();
        check_channels::<P>(channels)?;
        let mut image = Self::new(width, height)?;
        // `iter()` percorre na ordem lógica [H, W, C], qualquer que seja o layout.
        for (dst, src) in image.as_subpixels_mut().iter_mut().zip(array.iter()) {
            *dst = *src;
        }
        Ok(image)
    }

    /// Copia a imagem para um array `[H, W, C]` dono dos dados.
    #[must_use]
    pub fn to_array3(&self) -> Array3<P::Subpixel> {
        self.view().as_array3().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Error, Image, ImageView, Rect, Rgb8, L8};
    use ndarray::{Array3, Axis, Slice};
    use proptest::prelude::*;

    #[test]
    fn view_over_standard_layout_array_is_zero_copy() {
        let array = Array3::from_shape_fn((2, 3, 3), |(y, x, c)| {
            u8::try_from(y * 100 + x * 10 + c).unwrap()
        });
        let view = ImageView::<Rgb8>::from_array3(array.view()).unwrap();
        assert_eq!(view.dimensions(), (3, 2));
        assert_eq!(view.get(2, 1), Some(&Rgb8::new(120, 121, 122)));
        assert_eq!(view.row(0).as_ptr().cast::<u8>(), array.as_ptr());
    }

    #[test]
    fn array_with_wrong_channel_count_or_layout_is_rejected() {
        let gray = Array3::<u8>::zeros((2, 3, 1));
        assert!(matches!(
            ImageView::<Rgb8>::from_array3(gray.view()),
            Err(Error::ChannelMismatch {
                expected: 3,
                actual: 1
            })
        ));

        // Eixos invertidos: mesma forma lógica [H, W, 3], memória fora de ordem.
        let reversed = Array3::<u8>::zeros((3, 4, 2)).reversed_axes();
        assert!(matches!(
            ImageView::<Rgb8>::from_array3(reversed.view()),
            Err(Error::NonContiguous)
        ));
        // A cópia aceita qualquer layout.
        assert_eq!(
            Image::<Rgb8>::from_array3(reversed.view())
                .unwrap()
                .dimensions(),
            (4, 2)
        );

        let empty = Array3::<u8>::zeros((0, 3, 3));
        assert!(matches!(
            ImageView::<Rgb8>::from_array3(empty.view()),
            Err(Error::InvalidDimensions { .. })
        ));
    }

    #[test]
    fn sliced_array_needs_the_copying_constructor() {
        // Colunas 1..3 de um array 3×4×1: o `ndarray` não expõe a fatia de
        // memória de uma visão não contígua, então só a cópia é possível.
        let array = Array3::from_shape_fn((3, 4, 1), |(y, x, _)| u8::try_from(y * 10 + x).unwrap());
        // `slice_axis` em vez de `s![]`: a macro expande para `allow(unsafe_code)`,
        // incompatível com o `forbid` desta crate.
        let slice = array.slice_axis(Axis(1), Slice::from(1..3));
        assert!(matches!(
            ImageView::<L8>::from_array3(slice),
            Err(Error::NonContiguous)
        ));
        let image = Image::<L8>::from_array3(slice).unwrap();
        assert_eq!(image.dimensions(), (2, 3));
        assert_eq!(image.row(2), &[L8::new(21), L8::new(22)]);
    }

    #[test]
    fn as_array3_exposes_any_view_without_copy_using_strides() {
        let image = Image::from_fn(4, 3, |x, y| L8::new(u8::try_from(y * 4 + x).unwrap())).unwrap();
        let whole = image.view().as_array3();
        assert_eq!(whole.dim(), (3, 4, 1));
        assert_eq!(whole[[2, 3, 0]], 11);
        assert_eq!(whole.as_ptr(), image.as_subpixels().as_ptr());

        let roi = image.roi(Rect::new(1, 1, 2, 2)).unwrap();
        let array = roi.as_array3();
        assert_eq!(array.dim(), (2, 2, 1));
        assert_eq!(array.iter().copied().collect::<Vec<_>>(), [5, 6, 9, 10]);
        assert_eq!(
            array.as_ptr(),
            std::ptr::from_ref(&roi.row(0)[0]).cast::<u8>()
        );
    }

    #[test]
    fn owned_conversions_copy_in_row_major_order() {
        let array = Array3::from_shape_fn((2, 2, 3), |(y, x, c)| {
            u8::try_from(y * 6 + x * 3 + c).unwrap()
        });
        let image = Image::<Rgb8>::from_array3(array.view()).unwrap();
        assert_eq!(image[(1, 1)], Rgb8::new(9, 10, 11));
        assert_eq!(image.to_array3(), array);
    }

    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)] // centenas de casos: lento demais sob interpretação
        fn round_trip_preserves_every_subpixel(w in 1usize..12, h in 1usize..12, seed in any::<u8>()) {
            let array = Array3::from_shape_fn((h, w, 3), |(y, x, c)| {
                seed.wrapping_add(u8::try_from((y * 7 + x * 3 + c) % 256).unwrap())
            });
            let image = Image::<Rgb8>::from_array3(array.view()).unwrap();
            prop_assert_eq!(image.to_array3(), array.clone());
            prop_assert_eq!(image.view().as_array3(), array.view());
        }
    }
}
