//! Propagação de erros de sistemas conforme a [`ErrorPolicy`].

use bevy_ecs::prelude::*;
use perceptor_core::{Error, Result};
use perceptor_pipeline::frame::{Frame, FrameMeta};
use perceptor_pipeline::{fallible, ErrorPolicy, Pipeline, PipelineBuilder, PipelineErrors, Stage};

#[derive(Resource, Default)]
struct Seen(Vec<u64>);

fn frame(index: u64) -> Frame {
    let meta = FrameMeta {
        index,
        timestamp_us: 0,
        source: "test".into(),
    };
    Frame::new(meta, ndarray::Array3::zeros((2, 2, 1)))
}

fn always_fails() -> Result<()> {
    Err(Error::NonContiguous)
}

/// Falha só no frame de índice 1, como um kernel que rejeita uma entrada.
fn rejects_frame_one(frames: Query<(Entity, &Frame)>, mut errors: ResMut<PipelineErrors>) {
    for (entity, frame) in &frames {
        if frame.meta.index == 1 {
            errors.report_frame("rejects_frame_one", entity, Error::NonContiguous);
        }
    }
}

fn record_frames(frames: Query<&Frame>, mut seen: ResMut<Seen>) {
    seen.0.extend(frames.iter().map(|f| f.meta.index));
    seen.0.sort_unstable();
}

fn builder(policy: ErrorPolicy) -> PipelineBuilder {
    let mut builder = Pipeline::builder().with_error_policy(policy);
    builder.world_mut().init_resource::<Seen>();
    builder
}

#[test]
fn fail_fast_is_the_default_policy() {
    let pipeline = Pipeline::builder().build().unwrap();
    assert_eq!(pipeline.errors().policy(), ErrorPolicy::FailFast);
}

#[test]
fn fail_fast_returns_the_error_and_skips_later_stages() {
    let mut builder = builder(ErrorPolicy::FailFast);
    builder.add_system(Stage::Input, fallible(always_fails));
    builder.add_system(Stage::Output, |mut seen: ResMut<Seen>| seen.0.push(99));
    let mut pipeline = builder.build().unwrap();

    assert!(matches!(pipeline.tick(), Err(Error::NonContiguous)));
    assert!(
        pipeline.world().resource::<Seen>().0.is_empty(),
        "stages depois do erro não rodam em FailFast"
    );
}

#[test]
fn fail_fast_makes_run_return_the_error() {
    let mut builder = builder(ErrorPolicy::FailFast);
    builder.add_system(Stage::Process, fallible(always_fails));
    let mut pipeline = builder.with_max_ticks(5).build().unwrap();

    assert!(matches!(pipeline.run(), Err(Error::NonContiguous)));
    assert_eq!(pipeline.errors().total(), 1, "para no primeiro tick");
}

#[test]
fn continue_records_the_error_and_keeps_everything_running() {
    let mut builder = builder(ErrorPolicy::Continue);
    builder.add_system(Stage::Process, rejects_frame_one);
    builder.add_system(Stage::Output, record_frames);
    let mut pipeline = builder.build().unwrap();
    pipeline.world_mut().spawn(frame(0));
    let failing = pipeline.world_mut().spawn(frame(1)).id();

    pipeline.tick().unwrap();

    assert_eq!(pipeline.world().resource::<Seen>().0, [0, 1]);
    let errors: Vec<_> = pipeline.errors().iter().collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].system, "rejects_frame_one");
    assert_eq!(errors[0].frame, Some(failing));
    assert!(matches!(errors[0].error, Error::NonContiguous));
}

#[test]
fn skip_frame_discards_only_the_failing_frame_before_the_next_stage() {
    let mut builder = builder(ErrorPolicy::SkipFrame);
    builder.add_system(Stage::Process, rejects_frame_one);
    builder.add_system(Stage::PostProcess, record_frames);
    let mut pipeline = builder.build().unwrap();
    pipeline.world_mut().spawn(frame(0));
    pipeline.world_mut().spawn(frame(1));
    pipeline.world_mut().spawn(frame(2));

    pipeline.tick().unwrap();

    assert_eq!(pipeline.world().resource::<Seen>().0, [0, 2]);
    assert_eq!(pipeline.errors().len(), 1);
    let world = pipeline.world_mut();
    assert_eq!(world.query::<&Frame>().iter(world).count(), 2);
}

#[test]
fn skip_frame_keeps_the_pipeline_going_on_errors_without_a_frame() {
    let mut builder = builder(ErrorPolicy::SkipFrame);
    builder.add_system(Stage::Input, fallible(always_fails));
    builder.add_system(Stage::Output, |mut seen: ResMut<Seen>| seen.0.push(99));
    let mut pipeline = builder.with_max_ticks(3).build().unwrap();

    pipeline.run().unwrap();

    assert_eq!(pipeline.world().resource::<Seen>().0, [99, 99, 99]);
}

#[test]
fn errors_hold_the_last_tick_and_total_counts_all_of_them() {
    #[derive(Resource, Default)]
    struct Ticks(u64);

    let mut builder = builder(ErrorPolicy::Continue);
    builder.world_mut().init_resource::<Ticks>();
    // Falha nos dois primeiros ticks, não no terceiro.
    builder.add_system(
        Stage::Input,
        fallible(|mut ticks: ResMut<Ticks>| -> Result<()> {
            ticks.0 += 1;
            if ticks.0 <= 2 {
                return Err(Error::NonContiguous);
            }
            Ok(())
        }),
    );
    let mut pipeline = builder.build().unwrap();

    pipeline.tick().unwrap();
    pipeline.tick().unwrap();
    assert_eq!((pipeline.errors().len(), pipeline.errors().total()), (1, 2));

    pipeline.tick().unwrap();
    assert!(pipeline.errors().is_empty());
    assert_eq!(pipeline.errors().total(), 2);
}

#[test]
fn fallible_names_the_failing_system() {
    let mut builder = builder(ErrorPolicy::Continue);
    builder.add_system(Stage::Input, fallible(always_fails));
    let mut pipeline = builder.build().unwrap();

    pipeline.tick().unwrap();

    let error = pipeline.errors().iter().next().unwrap();
    assert!(error.system.ends_with("always_fails"), "{}", error.system);
    assert_eq!(error.frame, None);
}
