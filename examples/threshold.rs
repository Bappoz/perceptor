//! Limiarização global: limiar fixo, tipos e Otsu.
//!
//! ```text
//! cargo run --example threshold
//! ```

use perceptor::core::Result;
use perceptor::imgproc::threshold::{
    otsu_level, threshold, threshold_in_place, threshold_otsu, ThresholdType,
};
use perceptor::prelude::*;

/// Um objeto claro (≈ 180) sobre fundo escuro (≈ 40), com uma textura leve.
fn scene() -> Result<Image<L8>> {
    Image::from_fn(16, 8, |x, y| {
        let texture = u8::try_from((x * 7 + y * 3) % 16).unwrap_or(0);
        let inside = (4..12).contains(&x) && (2..6).contains(&y);
        L8::new(if inside { 180 + texture } else { 40 + texture })
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
    let image = scene()?;

    // Limiar fixo: acima de 100 vira 255, o resto vira 0.
    let binary = threshold(image.view(), 100, 255, ThresholdType::Binary)?;
    assert_eq!(count(&binary, 255), 8 * 4);
    assert_eq!(count(&binary, 0), 16 * 8 - 8 * 4);

    // Os outros tipos mantêm parte da imagem em vez de binarizar.
    let clipped = threshold(image.view(), 100, 0, ThresholdType::Trunc)?;
    assert!(clipped.as_pixels().iter().all(|p| p.luma() <= 100));

    // Otsu escolhe o limiar pelo histograma: cai no vão entre os dois modos.
    let level = otsu_level(image.view());
    println!("limiar de Otsu: {level}");
    assert!((55..180).contains(&level));
    let (used, by_otsu) = threshold_otsu(image.view(), 255, ThresholdType::Binary)?;
    assert_eq!(used, level);
    assert_eq!(by_otsu.as_pixels(), binary.as_pixels());

    // No pipeline: a versão no lugar entra em `add_op` por uma closure.
    let mut builder = Pipeline::builder();
    builder.add_op(Stage::Process, |frame: ImageViewMut<'_, L8>| {
        threshold_in_place(frame, 100, 255, ThresholdType::BinaryInv);
    });
    let mut pipeline = builder.build()?;
    let frame = pipeline
        .world_mut()
        .spawn(Frame::new(FrameMeta::new(0, "sintético"), image))
        .id();
    pipeline.tick()?;

    let inverted = pipeline
        .world()
        .get::<Frame>(frame)
        .and_then(|f| f.as_image::<L8>().ok())
        .map(|i| count(i, 0));
    println!("pixels do objeto, agora em 0: {inverted:?}");
    assert_eq!(inverted, Some(8 * 4));
    Ok(())
}
