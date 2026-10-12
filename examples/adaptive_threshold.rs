//! Limiarização adaptativa: o limiar vem da média local, não da imagem toda.
//!
//! ```text
//! cargo run --example adaptive_threshold
//! ```

use perceptor::core::Result;
use perceptor::imgproc::adaptive::{adaptive_threshold, AdaptiveMethod};
use perceptor::imgproc::threshold::{threshold_otsu, ThresholdType};
use perceptor::prelude::*;

const WIDTH: usize = 64;

/// Traços escuros (a cada 16 colunas) sobre um fundo que vai de 60 a 200:
/// a iluminação desigual de uma página fotografada com sombra.
fn page() -> Result<Image<L8>> {
    Image::from_fn(WIDTH, 16, |x, _| {
        let background = u8::try_from(60 + x * 140 / (WIDTH - 1)).unwrap_or(255);
        let stroke = matches!(x % 16, 7 | 8);
        L8::new(if stroke { background - 50 } else { background })
    })
}

fn is_stroke(x: usize) -> bool {
    matches!(x % 16, 7 | 8)
}

/// Pixels em que a máscara erra o que é traço (deve estar em 0) e o que é fundo.
fn errors(mask: &Image<L8>) -> usize {
    mask.as_pixels()
        .iter()
        .enumerate()
        .filter(|(i, p)| (p.luma() == 0) != is_stroke(i % WIDTH))
        .count()
}

fn main() -> Result<()> {
    let image = page()?;

    // Global: um limiar só não separa traço de fundo nos dois extremos da página.
    let (level, global) = threshold_otsu(image.view(), 255, ThresholdType::Binary)?;
    println!("Otsu ({level}): {} pixels errados", errors(&global));
    assert!(errors(&global) > 0);

    // Adaptativo: cada pixel contra a média da janela 15×15, menos c = 12.
    for method in [AdaptiveMethod::Mean, AdaptiveMethod::Gaussian] {
        let mask = adaptive_threshold(image.view(), 255, method, 15, 12)?;
        println!("{method:?}: {} pixels errados", errors(&mask));
        assert_eq!(errors(&mask), 0);
    }

    // Janela par é erro tipado, não pânico.
    assert!(adaptive_threshold(image.view(), 255, AdaptiveMethod::Mean, 14, 12).is_err());
    Ok(())
}
