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
//! Pixels fora da imagem valem zero (zero-padding).

use perceptor_core::{Image, ImageView, Result, L8};
use rayon::prelude::*;

use crate::f32_to_u8;

/// Kernels do operador Sobel 3×3.
const KERNEL_GX: [[i8; 3]; 3] = [[-1, 0, 1], [-2, 0, 2], [-1, 0, 1]];

const KERNEL_GY: [[i8; 3]; 3] = [[-1, -2, -1], [0, 0, 0], [1, 2, 1]];

/// Deslocamento de cada linha/coluna do kernel em relação ao pixel central.
const OFFSETS: [isize; 3] = [-1, 0, 1];

/// Aplica os kernels Sobel e retorna a magnitude do gradiente, saturada em `[0, 255]`.
///
/// # Errors
/// [`Error::Allocation`](perceptor_core::Error::Allocation) se a imagem de
/// saída não puder ser alocada.
pub fn apply_sobel(input: ImageView<'_, L8>) -> Result<Image<L8>> {
    let mut output = Image::<L8>::new(input.width(), input.height())?;

    // Valor do pixel, com zero-padding para coordenadas fora dos limites.
    let px = |y: usize, dy: isize, x: usize, dx: isize| -> i16 {
        match (y.checked_add_signed(dy), x.checked_add_signed(dx)) {
            (Some(yy), Some(xx)) => input.get(xx, yy).map_or(0, |p| i16::from(p.luma())),
            _ => 0,
        }
    };

    output
        .view_mut()
        .par_rows_mut()
        .enumerate()
        .for_each(|(y, row)| {
            for (x, out) in row.iter_mut().enumerate() {
                let (mut gx, mut gy) = (0i16, 0i16);

                for (i, &dy) in OFFSETS.iter().enumerate() {
                    for (j, &dx) in OFFSETS.iter().enumerate() {
                        let p = px(y, dy, x, dx);
                        gx += p * i16::from(KERNEL_GX[i][j]);
                        gy += p * i16::from(KERNEL_GY[i][j]);
                    }
                }

                // |gx|, |gy| ≤ 4·255, então os quadrados são exatos em f32.
                let (gx, gy) = (f32::from(gx), f32::from(gy));
                *out = L8::new(f32_to_u8((gx * gx + gy * gy).sqrt()));
            }
        });

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luma_at(image: &Image<L8>, x: usize, y: usize) -> u8 {
        image.get(x, y).unwrap().luma()
    }

    #[test]
    fn uniform_image_produces_zero_output() {
        // Imagem uniforme → gradiente zero nos pixels internos; as bordas
        // respondem por causa do zero-padding.
        let input = Image::filled(5, 5, L8::new(128)).unwrap();
        let out = apply_sobel(input.view()).unwrap();
        assert_eq!(out.dimensions(), (5, 5));
        for y in 1..4 {
            for x in 1..4 {
                assert_eq!(luma_at(&out, x, y), 0, "pixel ({y},{x}) deveria ser 0");
            }
        }
        assert!(luma_at(&out, 0, 0) > 0, "o zero-padding cria uma borda");
    }

    #[test]
    fn horizontal_edge_produces_high_response() {
        // Metade superior preta, metade inferior branca → borda horizontal nítida
        let (w, h) = (5, 10);
        let input = Image::from_fn(w, h, |_, y| L8::new(if y >= h / 2 { 255 } else { 0 })).unwrap();
        let out = apply_sobel(input.view()).unwrap();

        for x in 1..(w - 1) {
            let response = luma_at(&out, x, h / 2);
            assert!(
                response > 100,
                "resposta na borda deveria ser alta, got {response}"
            );
        }
    }

    #[test]
    fn vertical_step_matches_the_hand_computed_gradient() {
        // 0 0 10 10: no pixel (1, 1), Gx = (10 + 2·10 + 10) − 0 = 40 e Gy = 0.
        let input = Image::from_fn(4, 3, |x, _| L8::new(if x >= 2 { 10 } else { 0 })).unwrap();
        let out = apply_sobel(input.view()).unwrap();
        assert_eq!(luma_at(&out, 1, 1), 40);
        assert_eq!(luma_at(&out, 2, 1), 40);
    }

    #[test]
    fn output_shape_matches_input() {
        let input = Image::filled(13, 7, L8::new(42)).unwrap();
        let out = apply_sobel(input.view()).unwrap();
        assert_eq!(out.dimensions(), (13, 7));
    }
}
