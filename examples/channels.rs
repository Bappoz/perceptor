//! Conversões de layout de canais: RGB ↔ BGR, alpha, separar e juntar.
//!
//! ```text
//! cargo run --example channels
//! ```

use perceptor::core::Result;
use perceptor::imgproc::channels::{
    add_alpha, bgr_to_rgba, drop_alpha, merge_channels, rgb_to_bgr, split_channels, swap_channels,
};
use perceptor::prelude::*;

fn main() -> Result<()> {
    let rgb = Image::from_pixels(2, 1, &[Rgb8::new(255, 128, 0), Rgb8::new(10, 20, 30)])?;

    // RGB → BGR: a cor é a mesma, a ordem dos bytes se inverte.
    let bgr = rgb_to_bgr(rgb.view())?;
    assert_eq!(bgr.as_subpixels(), [0, 128, 255, 30, 20, 10]);
    assert_eq!(bgr.row(0)[0].r(), 255);

    // Alpha: acrescenta um canal constante; remover descarta, sem compor.
    let rgba = add_alpha(rgb.view(), 255)?;
    assert_eq!(rgba.row(0)[1], Rgba8::new(10, 20, 30, 255));
    assert_eq!(drop_alpha(rgba.view())?.as_pixels(), rgb.as_pixels());

    // Planos: um `Image<L8>` por canal, na ordem em memória.
    let planes = split_channels(rgb.view())?;
    assert_eq!(planes[1].as_pixels(), [L8::new(128), L8::new(20)]);
    let views: Vec<_> = planes.iter().map(Image::view).collect();
    assert_eq!(merge_channels::<Rgb8>(&views)?.as_pixels(), rgb.as_pixels());

    // No lugar: troca os bytes sem alocar. O tipo continua `Rgb8`.
    let mut buffer = rgb.clone();
    swap_channels(buffer.view_mut(), 0, 2)?;
    assert_eq!(buffer.as_subpixels(), bgr.as_subpixels());

    // No pipeline: a assinatura `ImageView<A> -> Result<Image<B>>` entra
    // direto em `add_op`; a que tem parâmetro extra entra por closure.
    let mut builder = Pipeline::builder();
    builder
        .add_op(Stage::PreProcess, rgb_to_bgr)
        .add_op(Stage::Process, |image: ImageView<'_, Bgr8>| {
            bgr_to_rgba(image, 255)
        });
    let mut pipeline = builder.build()?;
    let frame = pipeline
        .world_mut()
        .spawn(Frame::new(FrameMeta::new(0, "sintético"), rgb))
        .id();
    pipeline.tick()?;

    let format = pipeline.world().get::<Frame>(frame).map(Frame::format);
    println!("frame depois do pipeline: {format:?}");
    assert_eq!(format, Some(PixelFormat::Rgba8));
    Ok(())
}
