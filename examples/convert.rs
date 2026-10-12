//! Profundidade e normalização: do byte ao tensor de entrada de uma rede.
//!
//! ```text
//! cargo run --example convert
//! ```

use perceptor::core::Result;
use perceptor::imgproc::convert::{
    channel_mean_std, convert_depth, convert_scale, normalize_mean_std, normalize_minmax,
};
use perceptor::prelude::*;

/// Média e desvio da `ImageNet`, por canal RGB, em tons nominais.
const IMAGENET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const IMAGENET_STD: [f32; 3] = [0.229, 0.224, 0.225];

// As estatísticas vêm em `f64`; os parâmetros da normalização são `f32`.
#[allow(clippy::cast_possible_truncation)]
fn main() -> Result<()> {
    let image = Image::from_pixels(2, 1, &[Rgb8::new(0, 64, 255), Rgb8::new(255, 192, 0)])?;

    // Trocar de profundidade preserva o brilho: 255 → 1.0, e a volta é exata.
    let unit: Image<RgbF32> = convert_depth(image.view())?;
    let back: Image<Rgb8> = convert_depth(unit.view())?;
    assert_eq!(back.as_pixels(), image.as_pixels());
    println!("primeiro pixel em f32: {:?}", unit.as_pixels()[0].0);

    // Pré-processamento de rede: (v / 255 − média) / desvio, por canal.
    let tensor: Image<RgbF32> = normalize_mean_std(image.view(), &IMAGENET_MEAN, &IMAGENET_STD)?;
    println!("ImageNet: {:?}", tensor.as_pixels()[0].0);

    // Ou padronizar pelas estatísticas da própria imagem.
    let stats = channel_mean_std(image.view());
    let mean: Vec<f32> = stats.iter().map(|s| s.mean as f32).collect();
    let std: Vec<f32> = stats.iter().map(|s| s.std.max(1e-6) as f32).collect();
    let own: Image<RgbF32> = normalize_mean_std(image.view(), &mean, &std)?;
    println!("própria imagem: {:?}", own.as_pixels()[0].0);

    // Min-max leva o intervalo observado para [-1, 1].
    let signed: Image<RgbF32> = normalize_minmax(image.view(), -1.0, 1.0)?;
    assert_eq!(signed.as_pixels()[0].0[0], -1.0);
    assert_eq!(signed.as_pixels()[0].0[2], 1.0);

    // convert_scale é a conta crua `α·v + β`, com saturação no destino.
    let brighter: Image<Rgb8> = convert_scale(image.view(), 1.0, 100.0)?;
    assert_eq!(brighter.as_pixels()[0].0, [100, 164, 255]);
    Ok(())
}
