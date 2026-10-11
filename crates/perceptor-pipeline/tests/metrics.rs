//! `PipelineMetrics`: latência por stage, fps, frames descartados e spans.
#![cfg(feature = "metrics")]

use std::thread::sleep;
use std::time::Duration;

use bevy_ecs::prelude::*;
use perceptor_core::{Error, Image, L8};
use perceptor_pipeline::{ErrorPolicy, Frame, FrameMeta, Pipeline, PipelineErrors, Stage};

const WORK: Duration = Duration::from_millis(20);

fn frame(index: u64) -> Frame {
    Frame::new(
        FrameMeta::new(index, "test"),
        Image::<L8>::new(2, 2).unwrap(),
    )
}

/// Sistema de duração conhecida.
fn slow_system() {
    sleep(WORK);
}

#[test]
fn stage_latency_reflects_a_system_of_known_duration() {
    let mut builder = Pipeline::builder();
    builder.add_system(Stage::Process, slow_system);
    let mut pipeline = builder.build().unwrap();

    pipeline.tick().unwrap();

    let metrics = pipeline.metrics();
    let process = metrics.stage(Stage::Process);
    assert!(process.last >= WORK, "{:?}", process.last);
    assert_eq!(process.runs, 1);
    for stage in Stage::ALL {
        if stage != Stage::Process {
            let other = metrics.stage(stage);
            assert!(other.last < WORK, "{stage:?} levou {:?}", other.last);
            assert_eq!(other.runs, 1);
        }
    }
    assert!(metrics.tick().last >= WORK);
    let stages_sum: Duration = Stage::ALL.iter().map(|s| metrics.stage(*s).last).sum();
    assert!(stages_sum <= metrics.tick().last, "os stages cabem no tick");
}

#[test]
fn stage_statistics_accumulate_over_ticks() {
    let mut builder = Pipeline::builder();
    builder.add_system(Stage::Output, slow_system);
    let mut pipeline = builder.build().unwrap();

    for _ in 0..3 {
        pipeline.tick().unwrap();
    }

    let output = pipeline.metrics().stage(Stage::Output);
    assert_eq!(output.runs, 3);
    assert!(output.total >= WORK * 3);
    assert!(output.mean() >= WORK && output.mean() <= output.max);
    assert!(output.max >= output.last.min(output.max));
    assert_eq!(pipeline.metrics().tick().runs, 3);
}

#[test]
fn fps_is_bounded_by_the_known_tick_duration() {
    let mut builder = Pipeline::builder();
    // Um frame novo por tick, e 20 ms de trabalho: no máximo 50 fps.
    builder.add_system(Stage::Input, |mut commands: Commands| {
        commands.spawn(frame(0));
    });
    builder.add_system(Stage::Process, slow_system);
    builder.add_cleanup_system(perceptor_pipeline::despawn_processed_frames_system);
    let mut pipeline = builder.build().unwrap();
    assert_eq!(pipeline.metrics().fps(), None, "sem ticks não há taxa");

    for _ in 0..4 {
        pipeline.tick().unwrap();
    }

    let metrics = pipeline.metrics();
    assert_eq!(metrics.frames_processed(), 4);
    let fps = metrics.fps().unwrap();
    assert!(fps > 0.0 && fps <= 50.0, "fps = {fps}");
}

#[test]
fn frames_discarded_by_skip_frame_are_counted_once() {
    fn reject_odd(frames: Query<(Entity, &Frame)>, mut errors: ResMut<PipelineErrors>) {
        for (entity, frame) in &frames {
            if frame.meta.index % 2 == 1 {
                // Dois erros no mesmo frame contam como um descarte.
                errors.report_frame("reject_odd", entity, Error::NonContiguous);
                errors.report_frame("reject_odd", entity, Error::NonContiguous);
            }
        }
    }

    let mut builder = Pipeline::builder().with_error_policy(ErrorPolicy::SkipFrame);
    builder.add_system(Stage::PreProcess, reject_odd);
    let mut pipeline = builder.build().unwrap();
    for index in 0..5 {
        pipeline.world_mut().spawn(frame(index));
    }

    pipeline.tick().unwrap();
    pipeline.tick().unwrap();

    let metrics = pipeline.metrics();
    assert_eq!(metrics.frames_dropped(), 2);
    // Os três restantes chegam à saída nos dois ticks (nada os remove).
    assert_eq!(metrics.frames_processed(), 6);
}

#[test]
fn continue_policy_does_not_count_dropped_frames() {
    let mut builder = Pipeline::builder().with_error_policy(ErrorPolicy::Continue);
    builder.add_system(
        Stage::Process,
        |frames: Query<Entity, With<Frame>>, mut errors: ResMut<PipelineErrors>| {
            for entity in &frames {
                errors.report_frame("always", entity, Error::NonContiguous);
            }
        },
    );
    let mut pipeline = builder.build().unwrap();
    pipeline.world_mut().spawn(frame(0));

    pipeline.tick().unwrap();

    assert_eq!(pipeline.metrics().frames_dropped(), 0);
    assert_eq!(pipeline.metrics().frames_processed(), 1);
}
