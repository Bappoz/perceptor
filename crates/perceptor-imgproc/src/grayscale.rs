//! Conversão para escala de cinza.
//!
//! # Algoritmo
//! Usa os coeficientes de luminância BT.601 (padrão SDTV):
//! ```text
//! Y = 0.299·R + 0.587·G + 0.114·B
//! ```

use perceptor_core::{Image, ImageView, Result, Rgb8, L8};
use rayon::prelude::*;

use crate::f32_to_u8;

/// Converte uma imagem RGB para luminância.
///
/// Aceita qualquer visão, inclusive um recorte com stride maior que a linha.
///
/// # Errors
/// [`Error::Allocation`](perceptor_core::Error::Allocation) se a imagem de
/// saída não puder ser alocada.
pub fn convert_to_gray(input: ImageView<'_, Rgb8>) -> Result<Image<L8>> {
    let mut output = Image::<L8>::new(input.width(), input.height())?;

    output
        .view_mut()
        .par_rows_mut()
        .zip(input.par_rows())
        .for_each(|(dst, src)| {
            for (out, px) in dst.iter_mut().zip(src) {
                let luma = 0.299 * f32::from(px.r())
                    + 0.587 * f32::from(px.g())
                    + 0.114 * f32::from(px.b());
                *out = L8::new(f32_to_u8(luma));
            }
        });

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use perceptor_core::Rect;

    fn gray_of(pixel: Rgb8) -> u8 {
        let input = Image::filled(1, 1, pixel).unwrap();
        convert_to_gray(input.view()).unwrap().as_pixels()[0].luma()
    }

    #[test]
    fn white_pixel() {
        assert_eq!(gray_of(Rgb8::new(255, 255, 255)), 255);
    }

    #[test]
    fn black_pixel() {
        assert_eq!(gray_of(Rgb8::new(0, 0, 0)), 0);
    }

    #[test]
    fn pure_red() {
        // R=255, G=0, B=0 → Y = 0.299 * 255 ≈ 76
        assert_eq!(gray_of(Rgb8::new(255, 0, 0)), 76);
    }

    #[test]
    fn output_shape() {
        let input = Image::filled(6, 4, Rgb8::new(128, 128, 128)).unwrap();
        let out = convert_to_gray(input.view()).unwrap();
        assert_eq!(out.dimensions(), (6, 4));
    }

    #[test]
    fn accepts_a_non_contiguous_roi() {
        // Coluna x vale (x, 0, 0): o recorte começa em x = 1 e tem stride maior que a linha.
        let input =
            Image::from_fn(4, 3, |x, _| Rgb8::new(u8::try_from(x).unwrap() * 80, 0, 0)).unwrap();
        let roi = input.roi(Rect::new(1, 1, 2, 2)).unwrap();
        assert!(!roi.is_contiguous());

        let out = convert_to_gray(roi).unwrap();

        assert_eq!(out.dimensions(), (2, 2));
        let expected = [gray_of(Rgb8::new(80, 0, 0)), gray_of(Rgb8::new(160, 0, 0))];
        for row in out.rows() {
            assert_eq!([row[0].luma(), row[1].luma()], expected);
        }
    }
}
