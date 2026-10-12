//! Diferença entre quadros, máscara e mistura: aritmética saturada.
//!
//! ```text
//! cargo run --example arith
//! ```

use perceptor::core::Result;
use perceptor::imgproc::arith::{absdiff, add, add_weighted, bitwise_and};
use perceptor::imgproc::threshold::{threshold, ThresholdType};
use perceptor::prelude::*;

const SIDE: usize = 16;

/// Fundo cinza (50) com um quadrado claro (200) de 4×4 em `(x0, 6)`.
fn frame(x0: usize) -> Result<Image<L8>> {
    Image::from_fn(SIDE, SIDE, |x, y| {
        let inside = (x0..x0 + 4).contains(&x) && (6..10).contains(&y);
        L8::new(if inside { 200 } else { 50 })
    })
}

fn count(image: &Image<L8>, level: u8) -> usize {
    image
        .as_pixels()
        .iter()
        .filter(|p| p.luma() == level)
        .count()
}

fn main() -> Result<()> {
    let (before, after) = (frame(2)?, frame(8)?);

    // Movimento: |a − b| é 150 onde o quadrado saiu ou chegou, 0 no resto.
    let diff = absdiff(before.view(), after.view(), None)?;
    let moved = threshold(diff.view(), 30, 255, ThresholdType::Binary)?;
    assert_eq!(
        count(&moved, 255),
        2 * 4 * 4,
        "16 pixels saíram e 16 chegaram"
    );

    // Máscara: soma 100 de brilho só onde houve movimento; o resto fica igual.
    let boost = Image::filled(SIDE, SIDE, L8::new(100))?;
    let lit = add(before.view(), boost.view(), Some(moved.view()))?;
    assert_eq!(count(&lit, 255), 4 * 4, "200 + 100 satura em 255");
    assert_eq!(
        count(&lit, 50),
        SIDE * SIDE - 2 * 4 * 4,
        "fora da máscara, intocado"
    );

    // Recorte: `and` com a máscara (0 ou 255) mantém o quadro só no movimento.
    let cut = bitwise_and(before.view(), moved.view(), None)?;
    assert_eq!(count(&cut, 0), SIDE * SIDE - 2 * 4 * 4);
    assert_eq!(count(&cut, 200), 4 * 4, "o quadrado que saiu");

    // Mistura linear dos dois quadros: 0,5·a + 0,5·b.
    let mix = add_weighted(before.view(), 0.5, after.view(), 0.5, 0.0, None)?;
    assert_eq!(
        count(&mix, 125),
        2 * 4 * 4,
        "(50 + 200) / 2 onde só um tem o quadrado"
    );
    Ok(())
}
