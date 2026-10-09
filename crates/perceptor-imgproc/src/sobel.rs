//! Filtro de detecção de bordas via operador de Sobel.
//!
//! # Algoritmo
//! Aplica convolução 3×3 separada nos eixos X e Y, combinando com:
//! ```text
//! G = sqrt(Gx² + Gy²)   (magnitude do gradiente)
//! ```
//!
//! Kernels canônicos:
//! ```text
//! Gx = [[-1,  0,  1],   Gy = [[-1, -2, -1],
//!        [-2,  0,  2],         [ 0,  0,  0],
//!        [-1,  0,  1]]          [ 1,  2,  1]]
//! ```
//!
//! # Pré-requisito
//! A entrada **deve** ser grayscale (1 canal).

use ndarray::Array3;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::f32_to_u8;

/// Kernels do operador Sobel 3×3.
const KERNEL_GX: [[i8; 3]; 3] = [[-1, 0, 1], [-2, 0, 2], [-1, 0, 1]];

const KERNEL_GY: [[i8; 3]; 3] = [[-1, -2, -1], [0, 0, 0], [1, 2, 1]];

/// Deslocamento de cada linha/coluna do kernel em relação ao pixel central.
const OFFSETS: [isize; 3] = [-1, 0, 1];

/// Aplica os kernels Sobel e retorna a magnitude do gradiente `[H, W, 1]`.
///
/// # Panics
/// Panic se `input.shape()[2] != 1` (deve ser grayscale).
#[must_use]
pub fn apply_sobel(input: &Array3<u8>) -> Array3<u8> {
    assert_eq!(
        input.shape()[2],
        1,
        "Sobel requer frame grayscale [H, W, 1]"
    );
    let (h, w) = (input.shape()[0], input.shape()[1]);

    let src = input.as_slice().expect("input deve ser contíguo");

    // Retorna o valor do pixel com zero-padding para coordenadas fora dos limites.
    let px = |y: usize, dy: isize, x: usize, dx: isize| -> i16 {
        match (y.checked_add_signed(dy), x.checked_add_signed(dx)) {
            (Some(yy), Some(xx)) if yy < h && xx < w => i16::from(src[yy * w + xx]),
            _ => 0,
        }
    };

    // Calcula a magnitude do gradiente para cada pixel (ignora bordas).
    let flat: Vec<u8> = (0..h)
        .into_par_iter()
        .flat_map(|y| {
            (0..w)
                .map(move |x| {
                    let (mut gx, mut gy) = (0i16, 0i16);

                    for (i, &dy) in OFFSETS.iter().enumerate() {
                        for (j, &dx) in OFFSETS.iter().enumerate() {
                            let p = px(y, dy, x, dx);
                            gx += p * i16::from(KERNEL_GX[i][j]);
                            gy += p * i16::from(KERNEL_GY[i][j]);
                        }
                    }

                    // Magnitude do gradiente: sqrt(gx² + gy²), saturada em [0, 255].
                    // |gx|, |gy| ≤ 4·255, então os quadrados são exatos em f32.
                    let (gx, gy) = (f32::from(gx), f32::from(gy));
                    f32_to_u8((gx * gx + gy * gy).sqrt())
                })
                .collect::<Vec<u8>>()
        })
        .collect();

    Array3::from_shape_vec((h, w, 1), flat)
        .expect("shape deve ser compatível com o número de pixels")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_image_produces_zero_output() {
        // Imagem uniforme → gradiente zero em todo pixel interior
        // (bordas com zero-padding terão resposta, mas pixels internos = 0)
        let input = Array3::from_elem((5, 5, 1), 128u8);
        let out = apply_sobel(&input);
        assert_eq!(out.shape(), &[5, 5, 1]);
        // Pixels internos (longe das bordas do zero-padding) devem ser 0
        for y in 1..4usize {
            for x in 1..4usize {
                assert_eq!(out[[y, x, 0]], 0, "pixel ({y},{x}) deveria ser 0");
            }
        }
    }

    #[test]
    fn horizontal_edge_produces_high_response() {
        // Metade superior preta, metade inferior branca → borda horizontal nítida
        let h = 10;
        let w = 5;
        let mut flat = vec![0u8; h * w];
        for y in (h / 2)..h {
            for x in 0..w {
                flat[y * w + x] = 255;
            }
        }
        let input = Array3::from_shape_vec((h, w, 1), flat).unwrap();
        let out = apply_sobel(&input);

        let row = h / 2;
        for x in 1..(w - 1) {
            assert!(
                out[[row, x, 0]] > 100,
                "resposta na borda deveria ser alta, got {}",
                out[[row, x, 0]]
            );
        }
    }

    #[test]
    fn output_shape_matches_input() {
        let input = Array3::from_elem((7, 13, 1), 42u8);
        let out = apply_sobel(&input);
        assert_eq!(out.shape(), &[7, 13, 1]);
    }

    #[test]
    #[should_panic(expected = "Sobel requer frame grayscale")]
    fn panics_on_rgb_input() {
        let input = Array3::from_elem((4, 4, 3), 0u8);
        let _ = apply_sobel(&input);
    }
}
