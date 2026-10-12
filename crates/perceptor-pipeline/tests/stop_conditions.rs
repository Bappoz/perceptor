//! Condições de parada de `Pipeline::run`.

use std::time::{Duration, Instant};

use bevy_ecs::prelude::*;
use perceptor_core::{Image, L8};
use perceptor_pipeline::frame::{Frame, FrameMeta};
use perceptor_pipeline::plugins::io::IoPlugin;
use perceptor_pipeline::{
    despawn_processed_frames_system, ErrorPolicy, Pipeline, PipelineBuilder, PipelineState,
    Sources, Stage, StopReason,
};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cachorro.png");

/// Rede de segurança: um teste que regride para em `TickLimit` em vez de travar.
const SAFETY_TICKS: u64 = 10_000;

fn spawn_frames(per_tick: u64) -> impl FnMut(Commands, Local<u64>) {
    move |mut commands, mut index| {
        for _ in 0..per_tick {
            let image = Image::<L8>::new(2, 2).unwrap();
            commands.spawn(Frame::new(FrameMeta::new(*index, "sintético"), image));
            *index += 1;
        }
    }
}

fn state(pipeline: &Pipeline) -> &PipelineState {
    pipeline.world().resource::<PipelineState>()
}

fn guarded() -> PipelineBuilder {
    Pipeline::builder().with_max_ticks(SAFETY_TICKS)
}

#[test]
fn tick_limit_stops_after_n_ticks() {
    let mut pipeline = Pipeline::builder().with_max_ticks(3).build().unwrap();

    assert_eq!(pipeline.run().unwrap(), StopReason::TickLimit);

    assert_eq!(state(&pipeline).tick_count, 3);
    assert_eq!(pipeline.stop_reason(), Some(StopReason::TickLimit));
}

#[test]
fn frame_limit_stops_after_n_frames() {
    let mut builder = guarded().with_max_frames(5);
    builder.add_system(Stage::Input, spawn_frames(1));
    builder.add_system(Stage::Cleanup, despawn_processed_frames_system);
    let mut pipeline = builder.build().unwrap();

    assert_eq!(pipeline.run().unwrap(), StopReason::FrameLimit);

    assert_eq!(state(&pipeline).frame_count, 5);
    assert_eq!(state(&pipeline).tick_count, 5);
}

#[test]
fn frame_limit_never_cuts_a_tick_short() {
    let mut builder = guarded().with_max_frames(5);
    builder.add_system(Stage::Input, spawn_frames(2));
    let mut pipeline = builder.build().unwrap();

    assert_eq!(pipeline.run().unwrap(), StopReason::FrameLimit);

    assert_eq!(state(&pipeline).frame_count, 6);
    assert_eq!(state(&pipeline).tick_count, 3);
}

#[test]
fn frames_injected_between_ticks_are_counted() {
    let mut pipeline = Pipeline::builder().build().unwrap();
    let image = Image::<L8>::new(2, 2).unwrap();
    pipeline
        .world_mut()
        .spawn(Frame::new(FrameMeta::new(0, "manual"), image));

    pipeline.tick().unwrap();
    pipeline.tick().unwrap();

    assert_eq!(state(&pipeline).frame_count, 1);
}

#[test]
fn time_limit_stops_after_the_duration_has_elapsed() {
    let limit = Duration::from_millis(30);
    let mut builder = guarded().with_max_duration(limit);
    builder.add_system(Stage::Process, || {
        std::thread::sleep(Duration::from_millis(5));
    });
    let mut pipeline = builder.build().unwrap();

    let started = Instant::now();
    assert_eq!(pipeline.run().unwrap(), StopReason::TimeLimit);

    assert!(started.elapsed() >= limit);
    assert!(state(&pipeline).tick_count >= 2, "mais de um tick coube");
}

#[test]
fn zero_duration_still_runs_one_tick() {
    let mut pipeline = guarded().with_max_duration(Duration::ZERO).build().unwrap();

    assert_eq!(pipeline.run().unwrap(), StopReason::TimeLimit);
    assert_eq!(state(&pipeline).tick_count, 1);
}

#[test]
fn a_system_can_request_stop() {
    let mut builder = guarded();
    builder.add_system(Stage::Output, |mut state: ResMut<PipelineState>| {
        if state.tick_count == 3 {
            state.request_stop();
        }
    });
    let mut pipeline = builder.build().unwrap();

    assert_eq!(pipeline.run().unwrap(), StopReason::Requested);

    // `tick_count` só avança no fim do tick: o sistema viu 3 durante o quarto.
    assert_eq!(state(&pipeline).tick_count, 4);
    assert!(state(&pipeline).should_stop());
}

#[test]
fn stop_handle_stops_the_run_from_another_thread() {
    let mut builder = guarded();
    builder.add_system(Stage::Process, || {
        std::thread::sleep(Duration::from_millis(1));
    });
    let mut pipeline = builder.build().unwrap();
    let handle = pipeline.stop_handle();
    assert!(!handle.is_stopped());

    let stopper = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(10));
        handle.stop();
    });
    let reason = pipeline.run().unwrap();
    stopper.join().unwrap();

    assert_eq!(reason, StopReason::Requested);
    assert!(state(&pipeline).tick_count < SAFETY_TICKS);
    assert!(pipeline.stop_handle().is_stopped());
}

#[test]
fn finite_source_ends_the_run() {
    const NAME: &str = "contagem";

    let mut builder = guarded();
    builder.world_mut().resource_mut::<Sources>().register(NAME);
    builder.add_system(
        Stage::Input,
        |mut commands: Commands, mut sources: ResMut<Sources>, mut left: Local<Option<u64>>| {
            let left = left.get_or_insert(3);
            if *left == 0 {
                sources.finish(NAME);
                return;
            }
            *left -= 1;
            let image = Image::<L8>::new(2, 2).unwrap();
            commands.spawn(Frame::new(FrameMeta::new(*left, NAME), image));
        },
    );
    let mut pipeline = builder.build().unwrap();

    assert_eq!(pipeline.run().unwrap(), StopReason::EndOfStream);

    // Três ticks com frame e um em que a fonte descobriu o fim.
    assert_eq!(state(&pipeline).tick_count, 4);
    assert_eq!(state(&pipeline).frame_count, 3);
}

#[test]
fn run_waits_for_every_registered_source() {
    let mut builder = guarded();
    {
        let mut sources = builder.world_mut().resource_mut::<Sources>();
        sources.register("curta");
        sources.register("longa");
    }
    builder.add_system(
        Stage::Input,
        |mut sources: ResMut<Sources>, state: Res<PipelineState>| {
            sources.finish("curta");
            if state.tick_count == 4 {
                sources.finish("longa");
            }
        },
    );
    let mut pipeline = builder.build().unwrap();

    assert_eq!(pipeline.run().unwrap(), StopReason::EndOfStream);

    assert_eq!(state(&pipeline).tick_count, 5);
    let sources = pipeline.world().resource::<Sources>();
    assert!(sources.is_finished("curta") && sources.is_finished("longa"));
}

#[test]
fn without_sources_there_is_no_end_of_stream() {
    let mut pipeline = Pipeline::builder().with_max_ticks(4).build().unwrap();

    assert!(!pipeline.world().resource::<Sources>().all_finished());
    assert_eq!(pipeline.run().unwrap(), StopReason::TickLimit);
}

#[test]
fn first_condition_met_wins_and_is_kept() {
    let mut builder = Pipeline::builder().with_max_ticks(2).with_max_frames(100);
    builder.add_system(Stage::Input, spawn_frames(1));
    let mut pipeline = builder.build().unwrap();

    assert_eq!(pipeline.run().unwrap(), StopReason::TickLimit);
    // Um pipeline parado não roda mais ticks em `run()`.
    assert_eq!(pipeline.run().unwrap(), StopReason::TickLimit);
    assert_eq!(state(&pipeline).tick_count, 2);
}

#[test]
fn io_plugin_ends_the_run_without_extra_configuration() {
    let out = tempfile::tempdir().unwrap();
    let output = out.path().join("saida.png");
    let mut pipeline = guarded()
        .add_plugin(IoPlugin {
            input_path: FIXTURE.into(),
            output_path: output.clone(),
            ..Default::default()
        })
        .build()
        .unwrap();

    assert_eq!(pipeline.run().unwrap(), StopReason::EndOfStream);

    assert_eq!(state(&pipeline).tick_count, 1);
    assert_eq!(state(&pipeline).frame_count, 1);
    assert!(output.exists());
}

#[test]
fn io_plugin_reads_its_single_image_only_once() {
    let out = tempfile::tempdir().unwrap();
    let mut pipeline = Pipeline::builder()
        .add_plugin(IoPlugin {
            input_path: FIXTURE.into(),
            output_path: out.path().join("saida.png"),
            keep_frames: true,
            ..Default::default()
        })
        .build()
        .unwrap();

    pipeline.tick().unwrap();
    pipeline.tick().unwrap();

    assert_eq!(state(&pipeline).frame_count, 1);
    let world = pipeline.world_mut();
    assert_eq!(world.query::<&Frame>().iter(world).count(), 1);
}

#[test]
fn unreadable_source_ends_the_run_when_errors_do_not_halt() {
    let dir = tempfile::tempdir().unwrap();
    let mut pipeline = guarded()
        .with_error_policy(ErrorPolicy::Continue)
        .add_plugin(IoPlugin {
            input_path: dir.path().join("nao_existe.png"),
            output_path: dir.path().join("saida.png"),
            ..Default::default()
        })
        .build()
        .unwrap();

    assert_eq!(pipeline.run().unwrap(), StopReason::EndOfStream);

    assert_eq!(state(&pipeline).tick_count, 1);
    assert_eq!(pipeline.errors().total(), 1);
}
