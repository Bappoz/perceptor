//! Tabelas de consulta: gamma, ganho/offset e inversão.
//!
//! ```text
//! cargo run --example lut
//! ```

use perceptor::core::Result;
use perceptor::imgproc::lut::{apply_lut, apply_lut_in_place, apply_lut_per_channel, Lut};
use perceptor::prelude::*;

/// Gradiente escuro: de 0 a 120, subexposto.
fn dark() -> Result<Image<L8>> {
    Image::from_fn(16, 4, |x, _| L8::new(u8::try_from(x * 8).unwrap_or(255)))
}

/// Média dos níveis, em décimos para evitar `as f64` sobre contagens.
fn mean_tenths(image: &Image<L8>) -> u64 {
    let pixels = image.as_pixels();
    let sum: u64 = pixels.iter().map(|p| u64::from(p.luma())).sum();
    sum * 10 / u64::try_from(pixels.len()).unwrap_or(1)
}

fn main() -> Result<()> {
    let image = dark()?;

    // Gamma < 1 clareia os tons médios e deixa 0 e 255 onde estão.
    let bright = apply_lut(image.view(), &Lut::<u8>::gamma(0.5))?;
    println!(
        "média (décimos): {} → {}",
        mean_tenths(&image),
        mean_tenths(&bright)
    );
    assert!(mean_tenths(&bright) > mean_tenths(&image));

    // Ganho 2 estoura: tudo acima de 127 satura em 255.
    let gained = apply_lut(image.view(), &Lut::<u8>::gain_offset(2.0, 0.0))?;
    assert_eq!(gained.as_pixels().iter().map(|p| p.luma()).max(), Some(240));

    // Uma tabela por canal: só o vermelho é invertido.
    let rgb = Image::filled(2, 2, Rgb8::new(10, 20, 30))?;
    let tables = [Lut::<u8>::invert(), Lut::identity(), Lut::identity()];
    let out = apply_lut_per_channel(rgb.view(), &tables)?;
    assert_eq!(out.as_pixels()[0], Rgb8::new(245, 20, 30));

    // No pipeline: a versão no lugar entra em `add_op` com a tabela capturada.
    let table = Lut::<u8>::gamma(0.5);
    let mut builder = Pipeline::builder();
    builder.add_op(Stage::Process, move |frame: ImageViewMut<'_, L8>| {
        apply_lut_in_place(frame, &table);
    });
    let mut pipeline = builder.build()?;
    let frame = pipeline
        .world_mut()
        .spawn(Frame::new(FrameMeta::new(0, "sintético"), image))
        .id();
    pipeline.tick()?;
    let after = pipeline
        .world()
        .get::<Frame>(frame)
        .and_then(|f| f.as_image::<L8>().ok())
        .map(mean_tenths);
    println!("média no pipeline (décimos): {after:?}");
    assert_eq!(after, Some(mean_tenths(&bright)));
    Ok(())
}
