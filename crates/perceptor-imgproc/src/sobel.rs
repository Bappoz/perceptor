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
use perceptor_core::{Error, Result};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::f32_to_u8;

/// Kernels do operador Sobel 3×3.
const KERNEL_GX: [[i8; 3]; 3] = [[-1, 0, 1], [-2, 0, 2], [-1, 0, 1]];

const KERNEL_GY: [[i8; 3]; 3] = [[-1, -2, -1], [0, 0, 0], [1, 2, 1]];

/// Deslocamento de cada linha/coluna do kernel em relação ao pixel central.
const OFFSETS: [isize; 3] = [-1, 0, 1];

/// Aplica os kernels Sobel e retorna a magnitude do gradiente `[H, W, 1]`.
///
/// # Errors
/// - [`Error::ChannelMismatch`] se a entrada não tiver 1 canal.
/// - [`Error::NonContiguous`] se o tensor não estiver em ordem de linha.
///
/// # Panics
/// Não ocorre: a saída tem exatamente um valor por pixel de entrada.
pub fn apply_sobel(input: &Array3<u8>) -> Result<Array3<u8>> {
    let (h, w, c) = input.dim();
    if c != 1 {
        return Err(Error::ChannelMismatch {
            expected: 1,
            actual: c,
        });
    }

    let src = input.as_slice().ok_or(Error::NonContiguous)?;

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

    Ok(Array3::from_shape_vec((h, w, 1), flat).expect("um valor por pixel de entrada"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_image_produces_zero_output() {
        // Imagem uniforme → gradiente zero em todo pixel interior
        // (bordas com zero-padding terão resposta, mas pixels internos = 0)
        let input = Array3::from_elem((5, 5, 1), 128u8);
        let out = apply_sobel(&input).unwrap();
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
        let out = apply_sobel(&input).unwrap();

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
        let out = apply_sobel(&input).unwrap();
        assert_eq!(out.shape(), &[7, 13, 1]);
    }

    #[test]
    fn rejects_non_grayscale_input() {
        let input = Array3::from_elem((4, 4, 3), 0u8);
        assert!(matches!(
            apply_sobel(&input),
            Err(Error::ChannelMismatch {
                expected: 1,
                actual: 3
            })
        ));
    }
}
