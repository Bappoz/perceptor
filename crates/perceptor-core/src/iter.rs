//! Iteração por linha e por pixel, serial e paralela.
//!
//! Todos os iteradores respeitam o stride: o preenchimento entre linhas nunca
//! é lido nem escrito. As versões paralelas (feature `rayon`, ligada por
//! padrão) dividem o buffer com `par_chunks_mut`, então a disjunção entre
//! threads é garantida pelo verificador de empréstimos.

#[cfg(feature = "rayon")]
use rayon::prelude::*;

use crate::{Image, ImageView, ImageViewMut, Pixel};

impl<'a, P: Pixel> ImageView<'a, P> {
    /// Linhas de cima para baixo.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &'a [P]> + DoubleEndedIterator + Clone {
        let row_len = self.geometry.row_len();
        self.data
            .chunks(self.geometry.stride)
            .map(move |chunk| bytemuck::cast_slice(&chunk[..row_len]))
    }

    /// Pixels em ordem de linha.
    pub fn pixels(&self) -> impl Iterator<Item = &'a P> + Clone {
        self.rows().flatten()
    }

    /// Pixels em ordem de linha com suas coordenadas `(x, y, pixel)`.
    pub fn enumerate_pixels(&self) -> impl Iterator<Item = (usize, usize, &'a P)> + Clone {
        self.rows()
            .enumerate()
            .flat_map(|(y, row)| row.iter().enumerate().map(move |(x, pixel)| (x, y, pixel)))
    }

    /// Linhas em paralelo, indexadas de cima para baixo.
    #[cfg(feature = "rayon")]
    #[must_use]
    pub fn par_rows(&self) -> impl IndexedParallelIterator<Item = &'a [P]> {
        let row_len = self.geometry.row_len();
        self.data
            .par_chunks(self.geometry.stride)
            .map(move |chunk| bytemuck::cast_slice(&chunk[..row_len]))
    }
}

impl<P: Pixel> ImageViewMut<'_, P> {
    /// Linhas mutáveis de cima para baixo.
    pub fn rows_mut(&mut self) -> impl ExactSizeIterator<Item = &mut [P]> + DoubleEndedIterator {
        let row_len = self.geometry.row_len();
        self.data
            .chunks_mut(self.geometry.stride)
            .map(move |chunk| bytemuck::cast_slice_mut(&mut chunk[..row_len]))
    }

    /// Pixels mutáveis em ordem de linha.
    pub fn pixels_mut(&mut self) -> impl Iterator<Item = &mut P> {
        self.rows_mut().flatten()
    }

    /// Pixels mutáveis em ordem de linha com suas coordenadas `(x, y, pixel)`.
    pub fn enumerate_pixels_mut(&mut self) -> impl Iterator<Item = (usize, usize, &mut P)> {
        self.rows_mut().enumerate().flat_map(|(y, row)| {
            row.iter_mut()
                .enumerate()
                .map(move |(x, pixel)| (x, y, pixel))
        })
    }

    /// Linhas mutáveis em paralelo; use `.enumerate()` para obter `y`.
    #[cfg(feature = "rayon")]
    pub fn par_rows_mut(&mut self) -> impl IndexedParallelIterator<Item = &mut [P]> {
        let row_len = self.geometry.row_len();
        self.data
            .par_chunks_mut(self.geometry.stride)
            .map(move |chunk| bytemuck::cast_slice_mut(&mut chunk[..row_len]))
    }

    /// Faixas horizontais mutáveis em paralelo, de `rows_per_strip` linhas
    /// cada (a última pode ser menor).
    ///
    /// Cada item é `(y0, faixa)`, onde `y0` é a linha da visão original em
    /// que a faixa começa. Kernels que precisam de vizinhança vertical usam
    /// faixas em vez de linhas isoladas.
    ///
    /// # Panics
    /// Se `rows_per_strip == 0`.
    #[cfg(feature = "rayon")]
    pub fn par_strips_mut(
        &mut self,
        rows_per_strip: usize,
    ) -> impl IndexedParallelIterator<Item = (usize, ImageViewMut<'_, P>)> {
        assert!(rows_per_strip > 0, "rows_per_strip deve ser maior que zero");
        let geometry = self.geometry;
        let stride = geometry.stride;
        self.data
            .par_chunks_mut(stride.saturating_mul(rows_per_strip))
            .enumerate()
            .map(move |(index, chunk)| {
                // Faixas cheias têm `n · stride` subpixels; a última termina
                // sem preenchimento. Nos dois casos o teto dá o nº de linhas.
                let strip = geometry.with_height(chunk.len().div_ceil(stride));
                let len = strip.required_len();
                let view = ImageViewMut {
                    data: &mut chunk[..len],
                    geometry: strip,
                };
                (index * rows_per_strip, view)
            })
    }
}

impl<P: Pixel> Image<P> {
    /// Linhas de cima para baixo.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &[P]> + DoubleEndedIterator + Clone {
        self.as_pixels().chunks_exact(self.width())
    }

    /// Linhas mutáveis de cima para baixo.
    pub fn rows_mut(&mut self) -> impl ExactSizeIterator<Item = &mut [P]> + DoubleEndedIterator {
        let width = self.width();
        self.as_pixels_mut().chunks_exact_mut(width)
    }

    /// Pixels em ordem de linha.
    pub fn pixels(&self) -> impl ExactSizeIterator<Item = &P> + DoubleEndedIterator + Clone {
        self.as_pixels().iter()
    }

    /// Pixels mutáveis em ordem de linha.
    pub fn pixels_mut(&mut self) -> impl ExactSizeIterator<Item = &mut P> + DoubleEndedIterator {
        self.as_pixels_mut().iter_mut()
    }

    /// Pixels em ordem de linha com suas coordenadas `(x, y, pixel)`.
    pub fn enumerate_pixels(&self) -> impl Iterator<Item = (usize, usize, &P)> + Clone {
        self.view().enumerate_pixels()
    }

    /// Pixels mutáveis em ordem de linha com suas coordenadas `(x, y, pixel)`.
    pub fn enumerate_pixels_mut(&mut self) -> impl Iterator<Item = (usize, usize, &mut P)> {
        self.rows_mut().enumerate().flat_map(|(y, row)| {
            row.iter_mut()
                .enumerate()
                .map(move |(x, pixel)| (x, y, pixel))
        })
    }

    /// Linhas mutáveis em paralelo; use `.enumerate()` para obter `y`.
    #[cfg(feature = "rayon")]
    pub fn par_rows_mut(&mut self) -> impl IndexedParallelIterator<Item = &mut [P]> {
        let width = self.width();
        self.as_pixels_mut().par_chunks_exact_mut(width)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "rayon")]
    use crate::Rgb8;
    use crate::{Image, ImageView, ImageViewMut, L8};
    use proptest::prelude::*;
    #[cfg(feature = "rayon")]
    use rayon::prelude::*;

    /// Buffer 3×3 com stride 5: dois subpixels de preenchimento (99) por linha.
    fn padded() -> Vec<u8> {
        vec![0, 1, 2, 99, 99, 10, 11, 12, 99, 99, 20, 21, 22]
    }

    fn pattern(x: usize, y: usize) -> L8 {
        L8::new(u8::try_from((x * 7 + y * 13) % 251).unwrap())
    }

    #[test]
    fn rows_skip_padding_and_know_their_length() {
        let data = padded();
        let view = ImageView::<L8>::new(&data, 3, 3, 5).unwrap();
        let rows = view.rows();
        assert_eq!(rows.len(), 3);
        let collected: Vec<Vec<u8>> = rows.map(|r| r.iter().map(|p| p.luma()).collect()).collect();
        assert_eq!(collected, [[0, 1, 2], [10, 11, 12], [20, 21, 22]]);
        assert_eq!(view.rows().next_back().unwrap()[0], L8::new(20));
    }

    #[test]
    fn pixels_and_enumerate_follow_row_major_order() {
        let data = padded();
        let view = ImageView::<L8>::new(&data, 3, 3, 5).unwrap();
        let values: Vec<u8> = view.pixels().map(|p| p.luma()).collect();
        assert_eq!(values, [0, 1, 2, 10, 11, 12, 20, 21, 22]);

        let coords: Vec<(usize, usize, u8)> = view
            .enumerate_pixels()
            .map(|(x, y, p)| (x, y, p.luma()))
            .collect();
        assert_eq!(coords[0], (0, 0, 0));
        assert_eq!(coords[2], (2, 0, 2));
        assert_eq!(coords[3], (0, 1, 10));
        assert_eq!(coords[8], (2, 2, 22));
    }

    #[test]
    fn mutable_iterators_never_touch_padding() {
        let mut data = padded();
        let mut view = ImageViewMut::<L8>::new(&mut data, 3, 3, 5).unwrap();
        for row in view.rows_mut() {
            row[0] = L8::new(200);
        }
        for pixel in view.pixels_mut() {
            pixel.0[0] = pixel.0[0].saturating_add(1);
        }
        for (x, y, pixel) in view.enumerate_pixels_mut() {
            if (x, y) == (2, 2) {
                *pixel = L8::new(0);
            }
        }
        assert_eq!(data, [201, 2, 3, 99, 99, 201, 12, 13, 99, 99, 201, 22, 0]);
    }

    #[test]
    fn image_iterators_match_the_view_ones() {
        let mut img = Image::from_fn(4, 3, pattern).unwrap();
        assert_eq!(img.rows().len(), 3);
        assert!(img.rows().eq(img.view().rows()));
        assert!(img.pixels().eq(img.view().pixels()));
        assert_eq!(
            img.enumerate_pixels().nth(5).unwrap(),
            (1, 1, &pattern(1, 1))
        );

        for (y, row) in img.rows_mut().enumerate() {
            row.fill(L8::new(u8::try_from(y).unwrap()));
        }
        assert_eq!(img.as_subpixels(), &[0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2]);
        for (x, _, pixel) in img.enumerate_pixels_mut() {
            pixel.0[0] += u8::try_from(x).unwrap();
        }
        assert_eq!(
            img.row(2),
            &[L8::new(2), L8::new(3), L8::new(4), L8::new(5)]
        );
    }

    #[cfg(feature = "rayon")]
    #[test]
    fn strips_cover_every_row_exactly_once_and_report_their_origin() {
        let mut data = vec![0u8; 5 * 6 + 3]; // 3×7 com stride 5
        let mut view = ImageViewMut::<L8>::new(&mut data, 3, 7, 5).unwrap();
        let strips: Vec<(usize, usize)> = view
            .par_strips_mut(3)
            .map(|(y0, mut strip)| {
                strip.fill(L8::new(u8::try_from(y0).unwrap() + 1));
                (y0, strip.height())
            })
            .collect();
        assert_eq!(strips, [(0, 3), (3, 3), (6, 1)]);
        let firsts: Vec<u8> = view.as_view().rows().map(|r| r[0].luma()).collect();
        assert_eq!(firsts, [1, 1, 1, 4, 4, 4, 7]);
        // Preenchimento intacto.
        assert_eq!(data[3], 0);
        assert_eq!(data[4], 0);
    }

    #[cfg(feature = "rayon")]
    #[test]
    #[should_panic(expected = "rows_per_strip")]
    fn zero_rows_per_strip_is_a_programming_error() {
        let mut img = Image::<L8>::new(2, 2).unwrap();
        let _ = img.view_mut().par_strips_mut(0).count();
    }

    #[cfg(feature = "rayon")]
    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)] // rayon + centenas de casos: lento demais sob interpretação
        fn parallel_rows_and_strips_equal_the_serial_result(
            w in 1usize..24, h in 1usize..24, pad in 0usize..5, per_strip in 1usize..9
        ) {
            let stride = w * 3 + pad;
            let len = (h - 1) * stride + w * 3;
            let paint = |x: usize, y: usize| {
                let v = u8::try_from((x + 3 * y) % 256).unwrap();
                Rgb8::new(v, v.wrapping_add(1), v.wrapping_add(2))
            };

            let mut serial = vec![7u8; len];
            for (x, y, px) in ImageViewMut::<Rgb8>::new(&mut serial, w, h, stride)
                .unwrap()
                .enumerate_pixels_mut()
            {
                *px = paint(x, y);
            }

            let mut by_rows = vec![7u8; len];
            ImageViewMut::<Rgb8>::new(&mut by_rows, w, h, stride)
                .unwrap()
                .par_rows_mut()
                .enumerate()
                .for_each(|(y, row)| {
                    for (x, px) in row.iter_mut().enumerate() {
                        *px = paint(x, y);
                    }
                });

            let mut by_strips = vec![7u8; len];
            ImageViewMut::<Rgb8>::new(&mut by_strips, w, h, stride)
                .unwrap()
                .par_strips_mut(per_strip)
                .for_each(|(y0, mut strip)| {
                    for (x, y, px) in strip.enumerate_pixels_mut() {
                        *px = paint(x, y0 + y);
                    }
                });

            prop_assert_eq!(&serial, &by_rows);
            prop_assert_eq!(&serial, &by_strips);
        }
    }

    #[cfg(feature = "rayon")]
    #[test]
    fn image_parallel_rows_and_read_only_parallel_rows() {
        let mut img = Image::<L8>::new(5, 4).unwrap();
        img.par_rows_mut()
            .enumerate()
            .for_each(|(y, row)| row.fill(L8::new(u8::try_from(y).unwrap())));
        let sums: Vec<u32> = img
            .view()
            .par_rows()
            .map(|row| row.iter().map(|p| u32::from(p.luma())).sum())
            .collect();
        assert_eq!(sums, [0, 5, 10, 15]);
    }

    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)]
        fn iteration_visits_width_times_height_pixels(w in 1usize..20, h in 1usize..20, pad in 0usize..6) {
            let stride = w + pad;
            let data = vec![0u8; (h - 1) * stride + w];
            let view = ImageView::<L8>::new(&data, w, h, stride).unwrap();
            prop_assert_eq!(view.rows().len(), h);
            prop_assert_eq!(view.pixels().count(), w * h);
            prop_assert!(view.rows().all(|r| r.len() == w));
            let last = view.enumerate_pixels().last().unwrap();
            prop_assert_eq!((last.0, last.1), (w - 1, h - 1));
        }
    }
}
