//! Imagem dona do próprio buffer.
//!
//! [`Image<P>`] guarda `width × height` pixels do formato `P` em ordem de
//! linha, em um buffer contíguo cujo início é alinhado a 64 bytes (uma linha
//! de cache, e o maior alinhamento exigido por AVX-512).
//!
//! O alinhamento é obtido sem `unsafe`: o armazenamento é um `Vec` de blocos
//! `#[repr(align(64))]` reinterpretado como bytes e depois como pixels.

use std::fmt;
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::{Index, IndexMut};

use bytemuck::{Pod, Zeroable};

use crate::{Error, Pixel, Result};

/// Unidade de alocação: 64 bytes alinhados a 64.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C, align(64))]
struct Block([u8; 64]);

/// Imagem `width × height` de pixels `P`, dona do buffer.
///
/// Invariantes: `width > 0`, `height > 0` e o buffer contém exatamente
/// `width · height` pixels a partir de um endereço múltiplo de
/// [`Image::ALIGNMENT`].
#[derive(Clone)]
pub struct Image<P: Pixel> {
    blocks: Vec<Block>,
    width: usize,
    height: usize,
    _pixel: PhantomData<P>,
}

impl<P: Pixel> Image<P> {
    /// Alinhamento, em bytes, do início do buffer.
    pub const ALIGNMENT: usize = 64;

    /// Cria uma imagem com todos os subpixels em zero.
    ///
    /// # Errors
    /// - [`Error::InvalidDimensions`] se alguma dimensão for zero ou o tamanho
    ///   em bytes não couber em `isize`.
    /// - [`Error::Allocation`] se o alocador não puder atender ao pedido.
    pub fn new(width: usize, height: usize) -> Result<Self> {
        let bytes = Self::checked_byte_len(width, height)?;
        let block_count = bytes.div_ceil(size_of::<Block>());

        let mut blocks = Vec::new();
        blocks
            .try_reserve_exact(block_count)
            .map_err(|_| Error::Allocation { bytes })?;
        blocks.resize(block_count, Block::zeroed());

        Ok(Self {
            blocks,
            width,
            height,
            _pixel: PhantomData,
        })
    }

    /// Cria uma imagem com todos os pixels iguais a `value`.
    ///
    /// # Errors
    /// Os mesmos de [`Image::new`].
    pub fn filled(width: usize, height: usize, value: P) -> Result<Self> {
        let mut image = Self::new(width, height)?;
        image.as_pixels_mut().fill(value);
        Ok(image)
    }

    /// Cria uma imagem chamando `f(x, y)` para cada pixel, em ordem de linha.
    ///
    /// # Errors
    /// Os mesmos de [`Image::new`].
    pub fn from_fn(
        width: usize,
        height: usize,
        mut f: impl FnMut(usize, usize) -> P,
    ) -> Result<Self> {
        let mut image = Self::new(width, height)?;
        for (y, row) in image.as_pixels_mut().chunks_exact_mut(width).enumerate() {
            for (x, pixel) in row.iter_mut().enumerate() {
                *pixel = f(x, y);
            }
        }
        Ok(image)
    }

    /// Cria uma imagem a partir de pixels em ordem de linha.
    ///
    /// Os pixels são copiados para um buffer alinhado; um `Vec` comum não
    /// garante o alinhamento de [`Image::ALIGNMENT`].
    ///
    /// # Errors
    /// - [`Error::DimensionMismatch`] se `pixels.len() != width · height`.
    /// - Os mesmos de [`Image::new`].
    #[allow(clippy::needless_pass_by_value)] // simetria com `into_vec`; a posse sinaliza a cópia única
    pub fn from_vec(width: usize, height: usize, pixels: Vec<P>) -> Result<Self> {
        Self::from_pixels(width, height, &pixels)
    }

    /// Cria uma imagem copiando uma fatia de pixels em ordem de linha.
    ///
    /// # Errors
    /// Os mesmos de [`Image::from_vec`].
    pub fn from_pixels(width: usize, height: usize, pixels: &[P]) -> Result<Self> {
        let mut image = Self::new(width, height)?;
        let expected = image.as_pixels().len();
        if pixels.len() != expected {
            return Err(Error::DimensionMismatch {
                expected,
                actual: pixels.len(),
            });
        }
        image.as_pixels_mut().copy_from_slice(pixels);
        Ok(image)
    }

    /// Cria uma imagem copiando subpixels intercalados em ordem de linha.
    ///
    /// # Errors
    /// - [`Error::BufferLength`] se o comprimento não for múltiplo dos canais.
    /// - Os mesmos de [`Image::from_vec`].
    pub fn from_subpixels(width: usize, height: usize, subpixels: &[P::Subpixel]) -> Result<Self> {
        Self::from_pixels(width, height, P::cast_slice(subpixels)?)
    }

    /// Largura em pixels.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Altura em pixels.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// `(largura, altura)` em pixels.
    #[must_use]
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Distância, em pixels, entre o início de duas linhas consecutivas.
    ///
    /// Em uma imagem dona do buffer é sempre igual à largura; visões sobre
    /// buffers externos podem ter linhas com preenchimento.
    #[must_use]
    pub fn stride(&self) -> usize {
        self.width
    }

    /// Todos os pixels, em ordem de linha.
    #[must_use]
    pub fn as_pixels(&self) -> &[P] {
        // Nunca falha: o buffer é alinhado a 64 e tem um número inteiro de pixels.
        bytemuck::cast_slice(self.as_bytes())
    }

    /// Todos os pixels, mutáveis, em ordem de linha.
    #[must_use]
    pub fn as_pixels_mut(&mut self) -> &mut [P] {
        bytemuck::cast_slice_mut(self.as_bytes_mut())
    }

    /// Todos os subpixels intercalados, em ordem de linha.
    #[must_use]
    pub fn as_subpixels(&self) -> &[P::Subpixel] {
        P::as_subpixels(self.as_pixels())
    }

    /// Todos os subpixels intercalados, mutáveis.
    #[must_use]
    pub fn as_subpixels_mut(&mut self) -> &mut [P::Subpixel] {
        P::as_subpixels_mut(self.as_pixels_mut())
    }

    /// O buffer como bytes, sem o preenchimento do último bloco.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        let len = self.byte_len();
        &bytemuck::cast_slice(&self.blocks)[..len]
    }

    fn as_bytes_mut(&mut self) -> &mut [u8] {
        let len = self.byte_len();
        &mut bytemuck::cast_slice_mut(&mut self.blocks)[..len]
    }

    /// Linha `y`.
    ///
    /// # Panics
    /// Se `y >= height`.
    #[must_use]
    pub fn row(&self, y: usize) -> &[P] {
        assert!(y < self.height, "linha {y} fora dos limites");
        &self.as_pixels()[y * self.width..(y + 1) * self.width]
    }

    /// Linha `y`, mutável.
    ///
    /// # Panics
    /// Se `y >= height`.
    #[must_use]
    pub fn row_mut(&mut self, y: usize) -> &mut [P] {
        assert!(y < self.height, "linha {y} fora dos limites");
        let width = self.width;
        &mut self.as_pixels_mut()[y * width..(y + 1) * width]
    }

    /// Pixel em `(x, y)`, ou `None` fora dos limites.
    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> Option<&P> {
        let index = self.offset(x, y)?;
        self.as_pixels().get(index)
    }

    /// Pixel mutável em `(x, y)`, ou `None` fora dos limites.
    #[must_use]
    pub fn get_mut(&mut self, x: usize, y: usize) -> Option<&mut P> {
        let index = self.offset(x, y)?;
        self.as_pixels_mut().get_mut(index)
    }

    /// Consome a imagem e devolve os pixels em ordem de linha.
    #[must_use]
    pub fn into_vec(self) -> Vec<P> {
        self.as_pixels().to_vec()
    }

    /// Índice linear de `(x, y)`; `x` é checado à parte para não cair na linha seguinte.
    fn offset(&self, x: usize, y: usize) -> Option<usize> {
        (x < self.width && y < self.height).then(|| y * self.width + x)
    }

    /// Tamanho em bytes; não estoura porque foi validado na construção.
    fn byte_len(&self) -> usize {
        self.width * self.height * size_of::<P>()
    }

    fn checked_byte_len(width: usize, height: usize) -> Result<usize> {
        let invalid = Error::InvalidDimensions { width, height };
        if width == 0 || height == 0 {
            return Err(invalid);
        }
        match width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(size_of::<P>()))
        {
            // Nenhuma alocação em Rust pode passar de `isize::MAX` bytes.
            Some(bytes) if isize::try_from(bytes).is_ok() => Ok(bytes),
            _ => Err(invalid),
        }
    }
}

impl<P: Pixel> Index<(usize, usize)> for Image<P> {
    type Output = P;

    fn index(&self, (x, y): (usize, usize)) -> &P {
        self.get(x, y)
            .unwrap_or_else(|| panic!("pixel ({x}, {y}) fora dos limites"))
    }
}

impl<P: Pixel> IndexMut<(usize, usize)> for Image<P> {
    fn index_mut(&mut self, (x, y): (usize, usize)) -> &mut P {
        self.get_mut(x, y)
            .unwrap_or_else(|| panic!("pixel ({x}, {y}) fora dos limites"))
    }
}

impl<P: Pixel> PartialEq for Image<P> {
    fn eq(&self, other: &Self) -> bool {
        self.dimensions() == other.dimensions() && self.as_pixels() == other.as_pixels()
    }
}

impl<P: Pixel> fmt::Debug for Image<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Image")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("format", &P::FORMAT)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bgr8, Error, La8, Rgb8, RgbF32, Rgba8, L16, L8, LF32};
    use proptest::prelude::*;

    fn is_aligned<P: Pixel>(img: &Image<P>) -> bool {
        img.as_bytes()
            .as_ptr()
            .addr()
            .is_multiple_of(Image::<P>::ALIGNMENT)
    }

    #[test]
    fn new_is_zeroed_with_requested_dimensions() {
        let img = Image::<Rgb8>::new(4, 3).unwrap();
        assert_eq!((img.width(), img.height()), (4, 3));
        assert_eq!(img.dimensions(), (4, 3));
        assert_eq!(img.stride(), 4);
        assert_eq!(img.as_pixels().len(), 12);
        assert_eq!(img.as_subpixels().len(), 36);
        assert_eq!(img.as_bytes().len(), 36);
        assert!(img.as_pixels().iter().all(|&p| p == Rgb8::default()));
    }

    #[test]
    fn every_constructor_returns_a_64_byte_aligned_buffer() {
        assert_eq!(Image::<L8>::ALIGNMENT, 64);
        assert!(is_aligned(&Image::<L8>::new(1, 1).unwrap()));
        assert!(is_aligned(&Image::<La8>::new(3, 5).unwrap()));
        assert!(is_aligned(&Image::<Bgr8>::new(7, 7).unwrap()));
        assert!(is_aligned(&Image::<L16>::new(9, 2).unwrap()));
        assert!(is_aligned(&Image::<LF32>::new(5, 5).unwrap()));
        assert!(is_aligned(&Image::<RgbF32>::new(33, 3).unwrap()));
        assert!(is_aligned(
            &Image::filled(5, 5, Rgba8::new(1, 2, 3, 4)).unwrap()
        ));
        assert!(is_aligned(
            &Image::from_fn(5, 5, |_, _| L8::new(1)).unwrap()
        ));
        assert!(is_aligned(
            &Image::from_vec(2, 1, vec![L8::new(1); 2]).unwrap()
        ));
        assert!(is_aligned(
            &Image::<Rgb8>::from_subpixels(1, 1, &[1, 2, 3]).unwrap()
        ));
        assert!(is_aligned(&Image::<Rgb8>::new(3, 3).unwrap().clone()));
    }

    #[test]
    fn filled_sets_every_pixel() {
        let px = Rgba8::new(1, 2, 3, 4);
        let img = Image::filled(3, 2, px).unwrap();
        assert!(img.as_pixels().iter().all(|&p| p == px));
        assert_eq!(&img.as_bytes()[..8], &[1, 2, 3, 4, 1, 2, 3, 4]);
    }

    #[test]
    fn from_fn_receives_x_then_y_in_row_major_order() {
        let img = Image::from_fn(3, 2, |x, y| L8::new(u8::try_from(y * 10 + x).unwrap())).unwrap();
        assert_eq!(img.as_subpixels(), &[0, 1, 2, 10, 11, 12]);
        assert_eq!(img[(2, 1)], L8::new(12));
        assert_eq!(img.row(1), &[L8::new(10), L8::new(11), L8::new(12)]);
    }

    #[test]
    fn from_vec_keeps_pixels_and_into_vec_returns_them() {
        let pixels = vec![Rgb8::new(1, 2, 3), Rgb8::new(4, 5, 6)];
        let img = Image::from_vec(2, 1, pixels.clone()).unwrap();
        assert_eq!(img.as_pixels(), &pixels[..]);
        assert_eq!(img.into_vec(), pixels);
    }

    #[test]
    fn from_vec_rejects_wrong_pixel_count() {
        assert!(matches!(
            Image::from_vec(2, 2, vec![L8::default(); 3]),
            Err(Error::DimensionMismatch {
                expected: 4,
                actual: 3
            })
        ));
    }

    #[test]
    fn from_subpixels_validates_both_length_conditions() {
        assert!(matches!(
            Image::<Rgb8>::from_subpixels(1, 1, &[0; 4]),
            Err(Error::BufferLength {
                len: 4,
                channels: 3
            })
        ));
        assert!(matches!(
            Image::<Rgb8>::from_subpixels(2, 1, &[0; 3]),
            Err(Error::DimensionMismatch {
                expected: 2,
                actual: 1
            })
        ));
    }

    #[test]
    fn zero_dimensions_are_rejected() {
        for (w, h) in [(0, 0), (0, 5), (5, 0)] {
            assert!(matches!(
                Image::<L8>::new(w, h),
                Err(Error::InvalidDimensions { width, height }) if (width, height) == (w, h)
            ));
        }
    }

    #[test]
    fn overflowing_dimensions_are_rejected_without_panicking() {
        assert!(matches!(
            Image::<Rgba8>::new(usize::MAX, 2),
            Err(Error::InvalidDimensions { .. })
        ));
        assert!(matches!(
            Image::<RgbF32>::new(usize::MAX / 4, 1),
            Err(Error::InvalidDimensions { .. })
        ));
    }

    #[test]
    // O interpretador aborta com "resource exhaustion" em vez de devolver falha ao alocador.
    #[cfg_attr(miri, ignore)]
    fn allocation_failure_is_an_error_not_an_abort() {
        // 2^62 bytes: não estoura usize, mas nenhum alocador atende.
        assert!(matches!(
            Image::<Rgba8>::new(1 << 40, 1 << 20),
            Err(Error::Allocation { .. } | Error::InvalidDimensions { .. })
        ));
    }

    #[test]
    fn get_is_bounds_checked() {
        let mut img = Image::<L8>::new(2, 2).unwrap();
        *img.get_mut(1, 1).unwrap() = L8::new(9);
        assert_eq!(img.get(1, 1), Some(&L8::new(9)));
        assert_eq!(img.get(2, 0), None);
        assert_eq!(img.get(0, 2), None);
        assert!(img.get_mut(2, 2).is_none());
    }

    #[test]
    #[should_panic(expected = "fora dos limites")]
    fn index_panics_out_of_bounds_instead_of_wrapping_to_next_row() {
        let img = Image::<L8>::new(2, 2).unwrap();
        let _ = img[(2, 0)];
    }

    #[test]
    fn mutable_views_write_to_the_same_buffer() {
        let mut img = Image::<Rgb8>::new(2, 1).unwrap();
        img.as_pixels_mut()[1] = Rgb8::new(7, 8, 9);
        img.as_subpixels_mut()[0] = 1;
        img.row_mut(0)[0].channels_mut()[1] = 2;
        img[(0, 0)].channels_mut()[2] = 3;
        assert_eq!(img.as_bytes(), &[1, 2, 3, 7, 8, 9]);
    }

    #[test]
    fn clone_is_deep_and_equality_compares_pixels() {
        let a = Image::filled(2, 2, L16::new(500)).unwrap();
        let mut b = a.clone();
        assert_eq!(a, b);
        b[(0, 0)] = L16::new(1);
        assert_ne!(a, b);
        assert_ne!(a, Image::filled(4, 1, L16::new(500)).unwrap());
    }

    #[test]
    fn debug_shows_shape_and_format_not_pixels() {
        let text = format!("{:?}", Image::<Rgb8>::new(640, 480).unwrap());
        assert!(text.contains("640") && text.contains("480") && text.contains("Rgb8"));
        assert!(text.len() < 120);
    }

    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)] // centenas de casos: lento demais sob interpretação
        fn small_dimensions_succeed_exactly_when_non_zero(w in 0usize..48, h in 0usize..48) {
            match Image::<Rgb8>::new(w, h) {
                Ok(img) => {
                    prop_assert!(w > 0 && h > 0);
                    prop_assert_eq!(img.as_pixels().len(), w * h);
                    prop_assert!(is_aligned(&img));
                }
                Err(Error::InvalidDimensions { .. }) => prop_assert!(w == 0 || h == 0),
                Err(other) => prop_assert!(false, "erro inesperado: {other}"),
            }
        }

        #[test]
        #[cfg_attr(miri, ignore)]
        fn huge_dimensions_never_panic(w in (usize::MAX / 16)..=usize::MAX, h in 1usize..=usize::MAX) {
            prop_assert!(Image::<Rgba8>::new(w, h).is_err());
            prop_assert!(Image::<L8>::new(w, h.max(64)).is_err());
        }

        #[test]
        #[cfg_attr(miri, ignore)]
        fn from_fn_and_get_agree(w in 1usize..24, h in 1usize..24) {
            let img = Image::from_fn(w, h, |x, y| L16::new(u16::try_from(y * 100 + x).unwrap())).unwrap();
            for y in 0..h {
                for x in 0..w {
                    prop_assert_eq!(img.get(x, y).unwrap().luma(), u16::try_from(y * 100 + x).unwrap());
                }
            }
        }
    }
}
