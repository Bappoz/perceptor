//! Lendo as métricas do pipeline: latência por stage, fps e descartes.
//!
//! ```text
//! cargo run --example metrics
//! ```

use std::time::Duration;

use perceptor::core::Result;
use perceptor::imgproc::{grayscale::convert_to_gray, sobel::apply_sobel};
use perceptor::pipeline::despawn_processed_frames_system;
use perceptor::prelude::*;

/// Fonte sintética: um frame RGB 320×240 por tick.
fn source_system(mut commands: Commands, mut index: bevy_ecs::system::Local<u64>) -> Result<()> {
    let image = Image::from_fn(320, 240, |x, y| {
        let v = u8::try_from((x + y) % 256).unwrap_or(u8::MAX);
        Rgb8::new(v, v, v)
    })?;
    let timestamp = Duration::from_millis(*index * 33);
    let meta = FrameMeta::new(*index, "sintético").with_timestamp(timestamp);
    commands.spawn(Frame::new(meta, image));
    *index += 1;
    Ok(())
}

/// Processamento: escala de cinza seguida de Sobel.
fn edges_system(mut frames: Query<&mut Frame>) -> Result<()> {
    for mut frame in &mut frames {
        let gray = convert_to_gray(frame.as_image::<Rgb8>()?.view())?;
        frame.data = apply_sobel(gray.view())?.into();
    }
    Ok(())
}

fn main() -> Result<()> {
    let mut builder = Pipeline::builder().with_max_ticks(30);
    builder.add_system(Stage::Input, fallible(source_system));
    builder.add_system(Stage::Process, fallible(edges_system));
    builder.add_system(Stage::Cleanup, despawn_processed_frames_system);
    let mut pipeline = builder.build()?;

    pipeline.run()?;

    let metrics = pipeline.metrics();
    println!(
        "{:<12} {:>10} {:>10} {:>10}",
        "stage", "média", "máx", "último"
    );
    for stage in Stage::ALL {
        let t = metrics.stage(stage);
        println!(
            "{:<12} {:>10.1?} {:>10.1?} {:>10.1?}",
            format!("{stage:?}"),
            t.mean(),
            t.max,
            t.last
        );
    }
    let tick = metrics.tick();
    println!(
        "{:<12} {:>10.1?} {:>10.1?} {:>10.1?}",
        "tick",
        tick.mean(),
        tick.max,
        tick.last
    );
    println!(
        "{} frames, {} descartados, {:.1} fps",
        metrics.frames_processed(),
        metrics.frames_dropped(),
        metrics.fps().unwrap_or_default()
    );

    assert_eq!(metrics.frames_processed(), 30);
    assert!(metrics.stage(Stage::Process).mean() > metrics.stage(Stage::Output).mean());
    Ok(())
}
