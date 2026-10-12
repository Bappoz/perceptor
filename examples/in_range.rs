//! Segmentação por faixa de cor: `in_range` em HSV, com o matiz que dá a volta.
//!
//! ```text
//! cargo run --example in_range
//! ```

use perceptor::core::{Hsv8, Result};
use perceptor::imgproc::arith::bitwise_and;
use perceptor::imgproc::hsv::rgb_to_hsv;
use perceptor::imgproc::range::in_range;
use perceptor::prelude::*;

/// Fundo cinza com um disco vermelho e outro verde.
fn scene() -> Result<Image<Rgb8>> {
    Image::from_fn(48, 24, |x, y| {
        let inside = |cx: usize| x.abs_diff(cx).pow(2) + y.abs_diff(12).pow(2) < 64;
        if inside(12) {
            Rgb8::new(255, 0, 0)
        } else if inside(36) {
            Rgb8::new(0, 255, 0)
        } else {
            Rgb8::new(90, 90, 90)
        }
    })
}

fn on(mask: &Image<L8>) -> usize {
    mask.as_pixels().iter().filter(|p| p.luma() == 255).count()
}

fn main() -> Result<()> {
    let hsv = rgb_to_hsv::<Rgb8>(scene()?.view())?;

    // Em `Hsv8` o matiz vai de 0 a 179 e o vermelho fica dos dois lados do
    // zero: 170..=179 e 0..=10. `lower.h > upper.h` pede a volta.
    let red = in_range(hsv.view(), Hsv8::new(170, 100, 50), Hsv8::new(10, 255, 255))?;
    let green = in_range(hsv.view(), Hsv8::new(50, 100, 50), Hsv8::new(70, 255, 255))?;
    println!("pixels vermelhos: {}, verdes: {}", on(&red), on(&green));
    assert!(on(&red) > 0 && on(&green) > 0);
    assert_eq!(
        on(&bitwise_and(red.view(), green.view(), None)?),
        0,
        "disjuntas"
    );

    // Invertida em um canal que não é matiz, a faixa é vazia.
    let none = in_range(hsv.view(), Hsv8::new(0, 200, 0), Hsv8::new(179, 100, 255))?;
    assert_eq!(on(&none), 0);
    Ok(())
}
