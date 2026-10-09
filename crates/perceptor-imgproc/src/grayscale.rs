//! Conversão para escala de cinza.
//!
//! # Algoritmo
//! Usa os coeficientes de luminância BT.601 (padrão SDTV):
//! ```text
//! Y = 0.299·R + 0.587·G + 0.114·B
//! ```

use ndarray::Array3;
use rayon::prelude::*;

use crate::f32_to_u8;

/// Converte tensor RGB `[H, W, 3]` para luminância `[H, W, 1]`.
///
/// # Panics
/// Panic se `input.shape()[2] != 3`.
#[must_use]
pub fn convert_to_gray(input: &Array3<u8>) -> Array3<u8> {
    assert_eq!(input.shape()[2], 3, "esperado tensor RGB [H, W, 3]");
    let h = input.shape()[0];
    let w = input.shape()[1];

    let flat: Vec<u8> = input
        .as_slice()
        .expect("convert_to_gray: array não é contíguo")
        .par_chunks(3)
        .map(|px| {
            f32_to_u8(
                0.299 * f32::from(px[0]) + 0.587 * f32::from(px[1]) + 0.114 * f32::from(px[2]),
            )
        })
        .collect();

    Array3::from_shape_vec((h, w, 1), flat).expect("convert_to_gray: shape inválido")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_pixel() {
        let input = Array3::from_elem((1, 1, 3), 255u8);
        let out = convert_to_gray(&input);
        assert_eq!(out.shape(), &[1, 1, 1]);
        assert_eq!(out[[0, 0, 0]], 255);
    }

    #[test]
    fn black_pixel() {
        let input = Array3::zeros((1, 1, 3));
        let out = convert_to_gray(&input);
        assert_eq!(out[[0, 0, 0]], 0);
    }

    #[test]
    fn pure_red() {
        // R=255, G=0, B=0 → Y = 0.299 * 255 ≈ 76
        let mut input = Array3::zeros((1, 1, 3));
        input[[0, 0, 0]] = 255;
        let out = convert_to_gray(&input);
        assert_eq!(out[[0, 0, 0]], 76);
    }

    #[test]
    fn output_shape() {
        let input = Array3::from_elem((4, 6, 3), 128u8);
        let out = convert_to_gray(&input);
        assert_eq!(out.shape(), &[4, 6, 1]);
    }
}
