//! Histograma, máscara e equalização.
//!
//! ```text
//! cargo run --example histogram
//! ```

use perceptor::core::Result;
use perceptor::imgproc::histogram::{equalization_lut, equalize, histogram};
use perceptor::imgproc::lut::apply_lut;
use perceptor::prelude::*;

/// Imagem de baixo contraste: níveis só entre 100 e 140.
fn dull() -> Result<Image<L8>> {
    Image::from_fn(32, 8, |x, y| {
        L8::new(100 + u8::try_from((x * 5 + y * 3) % 41).unwrap_or(0))
    })
}

fn range(image: &Image<L8>) -> (u8, u8) {
    let levels = image.as_pixels().iter().map(|p| p.luma());
    (levels.clone().min().unwrap_or(0), levels.max().unwrap_or(0))
}

fn main() -> Result<()> {
    let image = dull()?;

    // A soma do histograma é o número de pixels.
    let counts = histogram(image.view(), 0, None)?;
    assert_eq!(counts.iter().sum::<u64>(), 32 * 8);
    println!("antes:  {:?}", range(&image));

    // Equalizar espalha os níveis por toda a faixa.
    let flat = equalize(image.view())?;
    println!("depois: {:?}", range(&flat));
    assert_eq!(range(&flat), (0, 255));

    // Com máscara: o histograma só da metade esquerda decide a tabela,
    // e a tabela é aplicada à imagem inteira.
    let mask = Image::from_fn(32, 8, |x, _| L8::new(if x < 16 { 255 } else { 0 }))?;
    let left = histogram(image.view(), 0, Some(mask.view()))?;
    assert_eq!(left.iter().sum::<u64>(), 16 * 8);
    let by_left = apply_lut(image.view(), &equalization_lut(&left))?;
    println!("pela esquerda: {:?}", range(&by_left));
    Ok(())
}
