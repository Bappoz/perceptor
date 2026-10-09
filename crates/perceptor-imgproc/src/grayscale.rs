//! Conversão para escala de cinza.
//!
//! # Algoritmo
//! Usa os coeficientes de luminância BT.601 (padrão SDTV):
//! ```text
//! Y = 0.299·R + 0.587·G + 0.114·B
//! ```

use ndarray::Array3;
use perceptor_core::{Error, Result};
use rayon::prelude::*;

use crate::f32_to_u8;

/// Converte tensor RGB `[H, W, 3]` para luminância `[H, W, 1]`.
///
/// # Errors
/// - [`Error::ChannelMismatch`] se a entrada não tiver 3 canais.
/// - [`Error::NonContiguous`] se o tensor não estiver em ordem de linha.
///
/// # Panics
/// Não ocorre: a saída tem exatamente um valor por pixel de entrada.
pub fn convert_to_gray(input: &Array3<u8>) -> Result<Array3<u8>> {
    let (h, w, c) = input.dim();
    if c != 3 {
        return Err(Error::ChannelMismatch {
            expected: 3,
            actual: c,
        });
    }

    let flat: Vec<u8> = input
        .as_slice()
        .ok_or(Error::NonContiguous)?
        .par_chunks(3)
        .map(|px| {
            f32_to_u8(
                0.299 * f32::from(px[0]) + 0.587 * f32::from(px[1]) + 0.114 * f32::from(px[2]),
            )
        })
        .collect();

    Ok(Array3::from_shape_vec((h, w, 1), flat).expect("um valor por pixel de entrada"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_pixel() {
        let input = Array3::from_elem((1, 1, 3), 255u8);
        let out = convert_to_gray(&input).unwrap();
        assert_eq!(out.shape(), &[1, 1, 1]);
        assert_eq!(out[[0, 0, 0]], 255);
    }

    #[test]
    fn black_pixel() {
        let input = Array3::zeros((1, 1, 3));
        let out = convert_to_gray(&input).unwrap();
        assert_eq!(out[[0, 0, 0]], 0);
    }

    #[test]
    fn pure_red() {
        // R=255, G=0, B=0 → Y = 0.299 * 255 ≈ 76
        let mut input = Array3::zeros((1, 1, 3));
        input[[0, 0, 0]] = 255;
        let out = convert_to_gray(&input).unwrap();
        assert_eq!(out[[0, 0, 0]], 76);
    }

    #[test]
    fn rejects_non_rgb_input() {
        let input = Array3::from_elem((2, 2, 1), 0u8);
        let err = convert_to_gray(&input).unwrap_err();
        assert!(matches!(
            err,
            Error::ChannelMismatch {
                expected: 3,
                actual: 1
            }
        ));
    }

    #[test]
    fn rejects_non_contiguous_input() {
        // Eixos invertidos: shape [4, 2, 3], mas a memória não está em ordem de linha.
        let input = Array3::from_elem((3, 2, 4), 0u8).reversed_axes();
        assert_eq!(input.shape(), &[4, 2, 3]);
        assert!(matches!(convert_to_gray(&input), Err(Error::NonContiguous)));
    }

    #[test]
    fn output_shape() {
        let input = Array3::from_elem((4, 6, 3), 128u8);
        let out = convert_to_gray(&input).unwrap();
        assert_eq!(out.shape(), &[4, 6, 1]);
    }
}
