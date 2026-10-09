//! Iteração serial e paralela sobre linhas e faixas.
//!
//! ```text
//! cargo run --release --example parallel_rows
//! ```

use perceptor::core::{Image, L8};
use rayon::prelude::*;

fn main() -> perceptor::core::Result<()> {
    let mut image = Image::<L8>::new(1920, 1080)?;

    // Serial: coordenadas junto de cada pixel.
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = L8::new(u8::try_from((x + y) % 256).unwrap_or(0));
    }

    // Paralelo por linha: cada thread recebe linhas inteiras e disjuntas.
    image.par_rows_mut().for_each(|row| {
        for pixel in row {
            *pixel = L8::new(255 - pixel.luma());
        }
    });

    // Paralelo por faixa de 64 linhas: `y0` é a linha em que a faixa começa.
    let rows_done: usize = image
        .view_mut()
        .par_strips_mut(64)
        .map(|(y0, mut strip)| {
            for (y, row) in strip.rows_mut().enumerate() {
                if (y0 + y) % 2 == 0 {
                    row.fill(L8::new(0));
                }
            }
            strip.height()
        })
        .sum();

    assert_eq!(rows_done, 1080);
    assert_eq!(image[(0, 0)], L8::new(0));
    assert_eq!(image[(0, 1)], L8::new(254));
    println!("{rows_done} linhas processadas em faixas");
    Ok(())
}
