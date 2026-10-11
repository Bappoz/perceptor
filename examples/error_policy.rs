//! Sistemas que falham: `fallible`, `report_frame` e `ErrorPolicy`.
//!
//! ```text
//! cargo run --example error_policy
//! ```

use perceptor::core::{Error, Result};
use perceptor::pipeline::frame::FrameMeta;
use perceptor::prelude::*;

/// Falha do sistema inteiro: retorna `Result` e é registrado com `fallible`.
fn check_config_system(frames: Query<&Frame>) -> Result<()> {
    if frames.is_empty() {
        return Err(Error::InvalidDimensions {
            width: 0,
            height: 0,
        });
    }
    Ok(())
}

/// Falha de um frame: reporta a entidade e segue para os próximos.
fn require_rgb_system(frames: Query<(Entity, &Frame)>, mut errors: ResMut<PipelineErrors>) {
    for (entity, frame) in &frames {
        if frame.channels() != 3 {
            let error = Error::ChannelMismatch {
                expected: 3,
                actual: frame.channels(),
            };
            errors.report_frame("require_rgb_system", entity, error);
        }
    }
}

fn spawn_frames(pipeline: &mut Pipeline) {
    for (index, channels) in [(0, 3), (1, 1), (2, 3)] {
        let meta = FrameMeta {
            index,
            timestamp_us: 0,
            source: "sintético".into(),
        };
        let data = Array3::zeros((2, 2, channels));
        pipeline.world_mut().spawn(Frame::new(meta, data));
    }
}

fn build(policy: ErrorPolicy) -> Pipeline {
    let mut builder = Pipeline::builder().with_error_policy(policy);
    builder.add_system(Stage::Input, fallible(check_config_system));
    builder.add_system(Stage::Process, require_rgb_system);
    let mut pipeline = builder.build();
    spawn_frames(&mut pipeline);
    pipeline
}

fn frame_count(pipeline: &mut Pipeline) -> usize {
    let world = pipeline.world_mut();
    world.query::<&Frame>().iter(world).count()
}

fn main() {
    // FailFast (padrão): `tick()` devolve o erro.
    let mut pipeline = build(ErrorPolicy::FailFast);
    let result = pipeline.tick();
    assert!(matches!(result, Err(Error::ChannelMismatch { .. })));
    println!("FailFast:  {result:?}");

    // SkipFrame: o frame de 1 canal sai, os outros dois seguem.
    let mut pipeline = build(ErrorPolicy::SkipFrame);
    pipeline.tick().expect("SkipFrame não interrompe o tick");
    for failure in pipeline.errors().iter() {
        println!("SkipFrame: {} → {}", failure.system, failure.error);
    }
    assert_eq!(frame_count(&mut pipeline), 2);

    // Continue: o erro fica registrado e os três frames permanecem.
    let mut pipeline = build(ErrorPolicy::Continue);
    pipeline.tick().expect("Continue não interrompe o tick");
    assert_eq!(pipeline.errors().len(), 1);
    assert_eq!(frame_count(&mut pipeline), 3);
    println!("Continue:  {} erro(s), 3 frames", pipeline.errors().len());
}
