//! Visões sem cópia sobre buffers de imagem.
//!
//! [`ImageView`] e [`ImageViewMut`] descrevem uma região retangular de pixels
//! dentro de um buffer de subpixels que pertence a outro dono: uma
//! [`Image`](crate::Image), um recorte dela, ou memória externa (decodificador,
//! câmera) cujas linhas podem ter preenchimento.
//!
//! O *stride* é medido em **subpixels**: a distância entre o início de duas
//! linhas consecutivas. Para formatos `u8` isso é o stride em bytes, e ele não
//! precisa ser múltiplo do tamanho do pixel.

use std::fmt;
use std::marker::PhantomData;
use std::ops::Range;

use crate::{Error, Image, Pixel, Result};

/// Retângulo em coordenadas de pixel, com origem no canto superior esquerdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect {
    /// Coluna do canto superior esquerdo.
    pub x: usize,
    /// Linha do canto superior esquerdo.
    pub y: usize,
    /// Largura em pixels.
    pub width: usize,
    /// Altura em pixels.
    pub height: usize,
}

impl Rect {
    /// Cria um retângulo `width × height` com canto superior esquerdo em `(x, y)`.
    #[must_use]
    pub const fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Primeira coluna à direita do retângulo, ou `None` em caso de overflow.
    #[must_use]
    pub const fn right(&self) -> Option<usize> {
        self.x.checked_add(self.width)
    }

    /// Primeira linha abaixo do retângulo, ou `None` em caso de overflow.
    #[must_use]
    pub const fn bottom(&self) -> Option<usize> {
        self.y.checked_add(self.height)
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}×{} em ({}, {})",
            self.width, self.height, self.x, self.y
        )
    }
}

/// Geometria validada de uma visão: toda linha `y < height` cabe no buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Geometry<P> {
    pub(crate) width: usize,
    pub(crate) height: usize,
    /// Subpixels entre o início de duas linhas consecutivas.
    pub(crate) stride: usize,
    _pixel: PhantomData<P>,
}

impl<P: Pixel> Geometry<P> {
    /// Valida a geometria contra um buffer de `len` subpixels.
    fn new(width: usize, height: usize, stride: usize, len: usize) -> Result<Self> {
        let invalid = Error::InvalidDimensions { width, height };
        if width == 0 || height == 0 {
            return Err(invalid);
        }
        let Some(row) = width.checked_mul(P::CHANNELS) else {
            return Err(invalid);
        };
        if stride < row {
            return Err(Error::InvalidStride { stride, min: row });
        }
        // A última linha não precisa do preenchimento.
        let Some(required) = (height - 1)
            .checked_mul(stride)
            .and_then(|start| start.checked_add(row))
        else {
            return Err(invalid);
        };
        if required > len {
            return Err(Error::BufferTooSmall {
                required,
                actual: len,
            });
        }
        Ok(Self {
            width,
            height,
            stride,
            _pixel: PhantomData,
        })
    }

    /// Mesma largura e stride, com outra altura. Quem chama garante que o
    /// buffer correspondente contém as `height` linhas.
    #[cfg(feature = "rayon")]
    pub(crate) fn with_height(self, height: usize) -> Self {
        Self { height, ..self }
    }

    /// Subpixels ocupados por uma linha, sem preenchimento.
    pub(crate) fn row_len(&self) -> usize {
        self.width * P::CHANNELS
    }

    /// Subpixels do início da primeira linha ao fim da última.
    pub(crate) fn required_len(&self) -> usize {
        (self.height - 1) * self.stride + self.row_len()
    }

    /// Faixa de subpixels da linha `y`.
    fn row_range(&self, y: usize) -> Range<usize> {
        assert!(y < self.height, "linha {y} fora dos limites");
        let start = y * self.stride;
        start..start + self.row_len()
    }

    fn contains(&self, x: usize, y: usize) -> bool {
        x < self.width && y < self.height
    }

    fn is_contiguous(&self) -> bool {
        self.stride == self.row_len()
    }

    /// Deslocamento e geometria de uma sub-região.
    fn roi(&self, region: Rect) -> Result<(usize, Self)> {
        if region.width == 0 || region.height == 0 {
            return Err(Error::InvalidDimensions {
                width: region.width,
                height: region.height,
            });
        }
        let inside = region.right().is_some_and(|right| right <= self.width)
            && region.bottom().is_some_and(|bottom| bottom <= self.height);
        if !inside {
            return Err(Error::OutOfBounds {
                region,
                width: self.width,
                height: self.height,
            });
        }
        let offset = region.y * self.stride + region.x * P::CHANNELS;
        let geometry = Self {
            width: region.width,
            height: region.height,
            ..*self
        };
        Ok((offset, geometry))
    }
}

/// Visão somente leitura de uma região de imagem.
#[derive(Debug, Clone, Copy)]
pub struct ImageView<'a, P: Pixel> {
    pub(crate) data: &'a [P::Subpixel],
    pub(crate) geometry: Geometry<P>,
}

impl<'a, P: Pixel> ImageView<'a, P> {
    /// Cria uma visão sobre subpixels intercalados com `stride` subpixels por linha.
    ///
    /// # Errors
    /// - [`Error::InvalidDimensions`] se alguma dimensão for zero ou a
    ///   geometria estourar `usize`.
    /// - [`Error::InvalidStride`] se `stride < width · canais`.
    /// - [`Error::BufferTooSmall`] se o buffer não contiver todas as linhas.
    pub fn new(
        data: &'a [P::Subpixel],
        width: usize,
        height: usize,
        stride: usize,
    ) -> Result<Self> {
        let geometry = Geometry::new(width, height, stride, data.len())?;
        Ok(Self {
            data: &data[..geometry.required_len()],
            geometry,
        })
    }

    /// Largura em pixels.
    #[must_use]
    pub fn width(&self) -> usize {
        self.geometry.width
    }

    /// Altura em pixels.
    #[must_use]
    pub fn height(&self) -> usize {
        self.geometry.height
    }

    /// `(largura, altura)` em pixels.
    #[must_use]
    pub fn dimensions(&self) -> (usize, usize) {
        (self.geometry.width, self.geometry.height)
    }

    /// Subpixels entre o início de duas linhas consecutivas.
    #[must_use]
    pub fn stride(&self) -> usize {
        self.geometry.stride
    }

    /// `true` se as linhas não têm preenchimento entre si.
    #[must_use]
    pub fn is_contiguous(&self) -> bool {
        self.geometry.is_contiguous()
    }

    /// Todos os pixels em uma única fatia, se a visão for contígua.
    #[must_use]
    pub fn as_pixels(&self) -> Option<&'a [P]> {
        self.is_contiguous()
            .then(|| bytemuck::cast_slice(self.data))
    }

    /// Linha `y`.
    ///
    /// # Panics
    /// Se `y >= height`.
    #[must_use]
    pub fn row(&self, y: usize) -> &'a [P] {
        // Nunca falha: a faixa tem `width · canais` subpixels.
        bytemuck::cast_slice(&self.data[self.geometry.row_range(y)])
    }

    /// Pixel em `(x, y)`, ou `None` fora dos limites.
    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> Option<&'a P> {
        self.geometry.contains(x, y).then(|| &self.row(y)[x])
    }

    /// Sub-região desta visão, sem cópia. Coordenadas relativas à visão.
    ///
    /// # Errors
    /// - [`Error::InvalidDimensions`] se a região tiver largura ou altura zero.
    /// - [`Error::OutOfBounds`] se a região não couber na visão.
    pub fn roi(&self, region: Rect) -> Result<Self> {
        let (offset, geometry) = self.geometry.roi(region)?;
        Ok(Self {
            data: &self.data[offset..offset + geometry.required_len()],
            geometry,
        })
    }

    /// Copia a região para uma [`Image`] compacta e alinhada.
    ///
    /// # Errors
    /// [`Error::Allocation`] se o alocador não atender ao pedido.
    pub fn to_image(&self) -> Result<Image<P>> {
        let mut image = Image::new(self.width(), self.height())?;
        for y in 0..self.height() {
            image.row_mut(y).copy_from_slice(self.row(y));
        }
        Ok(image)
    }
}

/// Visão mutável e exclusiva de uma região de imagem.
#[derive(Debug)]
pub struct ImageViewMut<'a, P: Pixel> {
    pub(crate) data: &'a mut [P::Subpixel],
    pub(crate) geometry: Geometry<P>,
}

impl<'a, P: Pixel> ImageViewMut<'a, P> {
    /// Cria uma visão mutável sobre subpixels intercalados.
    ///
    /// # Errors
    /// Os mesmos de [`ImageView::new`].
    pub fn new(
        data: &'a mut [P::Subpixel],
        width: usize,
        height: usize,
        stride: usize,
    ) -> Result<Self> {
        let geometry = Geometry::new(width, height, stride, data.len())?;
        Ok(Self {
            data: &mut data[..geometry.required_len()],
            geometry,
        })
    }

    /// Largura em pixels.
    #[must_use]
    pub fn width(&self) -> usize {
        self.geometry.width
    }

    /// Altura em pixels.
    #[must_use]
    pub fn height(&self) -> usize {
        self.geometry.height
    }

    /// `(largura, altura)` em pixels.
    #[must_use]
    pub fn dimensions(&self) -> (usize, usize) {
        (self.geometry.width, self.geometry.height)
    }

    /// Subpixels entre o início de duas linhas consecutivas.
    #[must_use]
    pub fn stride(&self) -> usize {
        self.geometry.stride
    }

    /// Visão somente leitura da mesma região.
    #[must_use]
    pub fn as_view(&self) -> ImageView<'_, P> {
        ImageView {
            data: self.data,
            geometry: self.geometry,
        }
    }

    /// Empresta a visão por um tempo menor, para operações que a consomem.
    #[must_use]
    pub fn reborrow(&mut self) -> ImageViewMut<'_, P> {
        ImageViewMut {
            data: self.data,
            geometry: self.geometry,
        }
    }

    /// Linha `y`.
    ///
    /// # Panics
    /// Se `y >= height`.
    #[must_use]
    pub fn row(&self, y: usize) -> &[P] {
        bytemuck::cast_slice(&self.data[self.geometry.row_range(y)])
    }

    /// Linha `y`, mutável.
    ///
    /// # Panics
    /// Se `y >= height`.
    #[must_use]
    pub fn row_mut(&mut self, y: usize) -> &mut [P] {
        bytemuck::cast_slice_mut(&mut self.data[self.geometry.row_range(y)])
    }

    /// Pixel em `(x, y)`, ou `None` fora dos limites.
    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> Option<&P> {
        self.geometry.contains(x, y).then(|| &self.row(y)[x])
    }

    /// Pixel mutável em `(x, y)`, ou `None` fora dos limites.
    #[must_use]
    pub fn get_mut(&mut self, x: usize, y: usize) -> Option<&mut P> {
        if self.geometry.contains(x, y) {
            Some(&mut self.row_mut(y)[x])
        } else {
            None
        }
    }

    /// Restringe a visão a uma sub-região. Use [`ImageViewMut::reborrow`]
    /// antes para continuar usando a visão original depois.
    ///
    /// # Errors
    /// Os mesmos de [`ImageView::roi`].
    pub fn into_roi(self, region: Rect) -> Result<Self> {
        let (offset, geometry) = self.geometry.roi(region)?;
        Ok(Self {
            data: &mut self.data[offset..offset + geometry.required_len()],
            geometry,
        })
    }

    /// Divide a visão em duas metades mutáveis disjuntas: linhas `0..y` e `y..height`.
    ///
    /// É a base do paralelismo por faixas: cada metade pode ir para uma thread.
    ///
    /// # Errors
    /// [`Error::OutOfBounds`] se `y == 0` ou `y >= height` (uma das metades
    /// ficaria vazia).
    pub fn split_at_row(self, y: usize) -> Result<(Self, Self)> {
        let Geometry { width, height, .. } = self.geometry;
        if y == 0 || y >= height {
            return Err(Error::OutOfBounds {
                region: Rect::new(0, y, width, height.saturating_sub(y)),
                width,
                height,
            });
        }
        let top = Geometry {
            height: y,
            ..self.geometry
        };
        let bottom = Geometry {
            height: height - y,
            ..self.geometry
        };
        let (top_data, bottom_data) = self.data.split_at_mut(y * self.geometry.stride);
        Ok((
            Self {
                data: &mut top_data[..top.required_len()],
                geometry: top,
            },
            Self {
                data: bottom_data,
                geometry: bottom,
            },
        ))
    }

    /// Atribui `value` a todos os pixels da região, sem tocar no preenchimento.
    pub fn fill(&mut self, value: P) {
        for y in 0..self.height() {
            self.row_mut(y).fill(value);
        }
    }

    /// Copia os pixels de `src` para esta região.
    ///
    /// # Errors
    /// [`Error::ShapeMismatch`] se as dimensões forem diferentes.
    pub fn copy_from(&mut self, src: &ImageView<'_, P>) -> Result<()> {
        if self.dimensions() != src.dimensions() {
            return Err(Error::ShapeMismatch {
                expected: self.dimensions(),
                actual: src.dimensions(),
            });
        }
        for y in 0..self.height() {
            self.row_mut(y).copy_from_slice(src.row(y));
        }
        Ok(())
    }
}

impl<P: Pixel> Image<P> {
    /// Visão somente leitura da imagem inteira.
    #[must_use]
    pub fn view(&self) -> ImageView<'_, P> {
        let geometry = self.geometry();
        ImageView {
            data: self.as_subpixels(),
            geometry,
        }
    }

    /// Visão mutável da imagem inteira.
    #[must_use]
    pub fn view_mut(&mut self) -> ImageViewMut<'_, P> {
        let geometry = self.geometry();
        ImageViewMut {
            data: self.as_subpixels_mut(),
            geometry,
        }
    }

    /// Sub-região da imagem, sem cópia.
    ///
    /// # Errors
    /// Os mesmos de [`ImageView::roi`].
    pub fn roi(&self, region: Rect) -> Result<ImageView<'_, P>> {
        self.view().roi(region)
    }

    /// Sub-região mutável da imagem, sem cópia.
    ///
    /// # Errors
    /// Os mesmos de [`ImageView::roi`].
    pub fn roi_mut(&mut self, region: Rect) -> Result<ImageViewMut<'_, P>> {
        self.view_mut().into_roi(region)
    }

    fn geometry(&self) -> Geometry<P> {
        Geometry {
            width: self.width(),
            height: self.height(),
            stride: self.stride(),
            _pixel: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, Image, Pixel, Rgb8, L8};
    use proptest::prelude::*;

    fn gradient(width: usize, height: usize) -> Image<L8> {
        Image::from_fn(width, height, |x, y| {
            L8::new(u8::try_from(y * 16 + x).unwrap())
        })
        .unwrap()
    }

    #[test]
    fn view_over_padded_rows_skips_the_padding() {
        // 2×2 com stride 3: o terceiro valor de cada linha é preenchimento.
        let data = [1u8, 2, 99, 3, 4];
        let view = ImageView::<L8>::new(&data, 2, 2, 3).unwrap();
        assert_eq!(view.dimensions(), (2, 2));
        assert_eq!(view.stride(), 3);
        assert_eq!(view.row(0), &[L8::new(1), L8::new(2)]);
        assert_eq!(view.row(1), &[L8::new(3), L8::new(4)]);
        assert_eq!(view.get(1, 1), Some(&L8::new(4)));
        assert_eq!(view.get(2, 0), None);
        assert_eq!(view.get(0, 2), None);
        assert!(!view.is_contiguous());
        assert!(view.as_pixels().is_none());
    }

    #[test]
    fn view_accepts_byte_stride_that_is_not_a_multiple_of_the_pixel_size() {
        // RGB 2 pixels de largura (6 bytes) com linhas alinhadas a 8 bytes.
        let mut data = [0u8; 16];
        data[8..14].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
        let view = ImageView::<Rgb8>::new(&data, 2, 2, 8).unwrap();
        assert_eq!(view.row(1), &[Rgb8::new(1, 2, 3), Rgb8::new(4, 5, 6)]);
    }

    #[test]
    fn view_is_zero_copy() {
        let data = [0u8; 12];
        let view = ImageView::<L8>::new(&data, 3, 3, 4).unwrap();
        assert_eq!(view.row(2).as_ptr().cast::<u8>(), data[8..].as_ptr());
    }

    #[test]
    fn new_validates_geometry_against_the_buffer() {
        let data = [0u8; 8];
        assert!(matches!(
            ImageView::<L8>::new(&data, 0, 2, 4),
            Err(Error::InvalidDimensions { .. })
        ));
        assert!(matches!(
            ImageView::<Rgb8>::new(&data, 2, 1, 5),
            Err(Error::InvalidStride { stride: 5, min: 6 })
        ));
        assert!(matches!(
            ImageView::<L8>::new(&data, 4, 3, 4),
            Err(Error::BufferTooSmall {
                required: 12,
                actual: 8
            })
        ));
        // A última linha não precisa do preenchimento: 4 + 3 = 7 ≤ 8.
        assert!(ImageView::<L8>::new(&data[..7], 3, 2, 4).is_ok());
        assert!(matches!(
            ImageView::<L8>::new(&data, 2, usize::MAX, usize::MAX),
            Err(Error::InvalidDimensions { .. })
        ));
    }

    #[test]
    fn image_view_exposes_the_same_pixels() {
        let img = gradient(4, 3);
        let view = img.view();
        assert_eq!(view.dimensions(), (4, 3));
        assert!(view.is_contiguous());
        assert_eq!(view.as_pixels(), Some(img.as_pixels()));
        assert_eq!(view.to_image().unwrap(), img);
    }

    #[test]
    fn roi_selects_a_sub_rectangle_without_copying() {
        let img = gradient(4, 4);
        let roi = img.roi(Rect::new(1, 2, 2, 2)).unwrap();
        assert_eq!(roi.dimensions(), (2, 2));
        assert_eq!(roi.row(0), &[L8::new(33), L8::new(34)]);
        assert_eq!(roi.row(1), &[L8::new(49), L8::new(50)]);
        assert_eq!(
            roi.row(0).as_ptr(),
            std::ptr::from_ref(img.get(1, 2).unwrap())
        );
        assert!(!roi.is_contiguous());

        // ROI de ROI: coordenadas relativas à view.
        let inner = roi.roi(Rect::new(1, 1, 1, 1)).unwrap();
        assert_eq!(inner.get(0, 0), Some(&L8::new(50)));

        let copy = roi.to_image().unwrap();
        assert_eq!(copy.as_subpixels(), &[33, 34, 49, 50]);
    }

    #[test]
    fn roi_rejects_regions_outside_the_image() {
        let img = gradient(4, 4);
        for rect in [
            Rect::new(3, 0, 2, 1),
            Rect::new(0, 3, 1, 2),
            Rect::new(4, 0, 1, 1),
            Rect::new(usize::MAX, 0, 2, 1),
        ] {
            assert!(matches!(
                img.roi(rect),
                Err(Error::OutOfBounds { region, width: 4, height: 4 }) if region == rect
            ));
        }
        assert!(matches!(
            img.roi(Rect::new(0, 0, 0, 1)),
            Err(Error::InvalidDimensions { .. })
        ));
        assert!(img.roi(Rect::new(0, 0, 4, 4)).is_ok());
    }

    #[test]
    fn mutable_view_writes_through_to_the_image() {
        let mut img = Image::<L8>::new(3, 3).unwrap();
        {
            let mut view = img.view_mut();
            view.row_mut(1)[1] = L8::new(5);
            *view.get_mut(2, 2).unwrap() = L8::new(9);
            assert!(view.get_mut(3, 0).is_none());
            assert_eq!(view.as_view().get(1, 1), Some(&L8::new(5)));
        }
        assert_eq!(img.as_subpixels(), &[0, 0, 0, 0, 5, 0, 0, 0, 9]);
    }

    #[test]
    fn fill_on_a_mutable_roi_leaves_the_rest_untouched() {
        let mut img = Image::<L8>::new(4, 3).unwrap();
        img.roi_mut(Rect::new(1, 1, 2, 2)).unwrap().fill(L8::new(7));
        assert_eq!(img.as_subpixels(), &[0, 0, 0, 0, 0, 7, 7, 0, 0, 7, 7, 0]);
    }

    #[test]
    fn copy_from_requires_equal_dimensions() {
        let src = gradient(2, 2);
        let mut dst = Image::<L8>::new(4, 4).unwrap();
        dst.roi_mut(Rect::new(2, 2, 2, 2))
            .unwrap()
            .copy_from(&src.view())
            .unwrap();
        assert_eq!(dst[(3, 3)], L8::new(17));
        assert_eq!(dst[(1, 1)], L8::new(0));

        assert!(matches!(
            dst.view_mut().copy_from(&src.view()),
            Err(Error::ShapeMismatch {
                expected: (4, 4),
                actual: (2, 2)
            })
        ));
    }

    #[test]
    fn split_at_row_gives_two_disjoint_mutable_halves() {
        let mut img = Image::<L8>::new(2, 4).unwrap();
        {
            let (mut top, mut bottom) = img.view_mut().split_at_row(1).unwrap();
            assert_eq!((top.height(), bottom.height()), (1, 3));
            top.fill(L8::new(1));
            bottom.fill(L8::new(2));
        }
        assert_eq!(img.as_subpixels(), &[1, 1, 2, 2, 2, 2, 2, 2]);

        assert!(matches!(
            img.view_mut().split_at_row(0),
            Err(Error::OutOfBounds { .. })
        ));
        assert!(matches!(
            img.view_mut().split_at_row(4),
            Err(Error::OutOfBounds { .. })
        ));
    }

    #[test]
    fn mutable_view_over_external_padded_buffer() {
        let mut data = [0u8; 7];
        let mut view = ImageViewMut::<L8>::new(&mut data, 3, 2, 4).unwrap();
        view.fill(L8::new(1));
        assert_eq!(data, [1, 1, 1, 0, 1, 1, 1]);
    }

    #[test]
    fn rect_reports_its_edges() {
        let rect = Rect::new(1, 2, 3, 4);
        assert_eq!((rect.right(), rect.bottom()), (Some(4), Some(6)));
        assert_eq!(Rect::new(usize::MAX, 0, 1, 1).right(), None);
        assert_eq!(rect.to_string(), "3×4 em (1, 2)");
    }

    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)] // centenas de casos: lento demais sob interpretação
        fn writing_to_a_roi_never_touches_pixels_outside_it(
            (w, h, x, y, rw, rh) in (1usize..20, 1usize..20).prop_flat_map(|(w, h)| {
                (0..w, 0..h).prop_flat_map(move |(x, y)| {
                    (Just(w), Just(h), Just(x), Just(y), 1..=w - x, 1..=h - y)
                })
            })
        ) {
            let mut img = Image::<Rgb8>::new(w, h).unwrap();
            let mark = Rgb8::new(1, 2, 3);
            img.roi_mut(Rect::new(x, y, rw, rh)).unwrap().fill(mark);

            for py in 0..h {
                for px in 0..w {
                    let inside = (x..x + rw).contains(&px) && (y..y + rh).contains(&py);
                    let expected = if inside { mark } else { Rgb8::default() };
                    prop_assert_eq!(img[(px, py)], expected);
                }
            }
        }

        #[test]
        #[cfg_attr(miri, ignore)]
        fn arbitrary_geometry_never_panics(
            len in 0usize..64, w in 0usize..12, h in 0usize..12, stride in 0usize..40
        ) {
            let data = vec![0u8; len];
            if let Ok(view) = ImageView::<Rgb8>::new(&data, w, h, stride) {
                prop_assert_eq!(view.row(h - 1).len(), w);
                prop_assert_eq!(view.to_image().unwrap().dimensions(), (w, h));
            }
        }
    }

    #[test]
    fn stride_is_measured_in_subpixels() {
        let img = Image::<Rgb8>::new(4, 2).unwrap();
        assert_eq!(img.stride(), 4 * Rgb8::CHANNELS);
        assert_eq!(img.view().stride(), 12);
    }
}
