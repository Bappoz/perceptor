//! Conversão para escala de cinza.
//!
//! # Algoritmo
//! Usa os coeficientes de luminância BT.601 (padrão SDTV):
//! ```text
//! Y = 0.299·R + 0.587·G + 0.114·B
//! ```

use ndarray::Array3;
use rayon::prelude::*;


/// Converte tensor RGB `[H, W, 3]` para luminância `[H, W, 1]`.
///
/// # Panics
/// Panic se `input.shape()[2] != 3`.
pub fn convert_to_gray(input: &Array3<u8>) -> Array3<u8> {
    assert_eq!(input.shape()[2], 3, "esperado tensor RGB [H, W, 3]");
    let h = input.shape()[0];
    let w = input.shape()[1];

    let flat: Vec<u8> = input
        .as_slice()
        .expect("convert_to_gray: array não é contíguo")
        .par_chunks(3)
        .map(|px| (0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32) as u8)
        .collect();

    Array3::from_shape_vec((h, w, 1), flat).expect("convert_to_gray: shape inválido")
}
