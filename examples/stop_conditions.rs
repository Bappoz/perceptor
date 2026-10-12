//! Condições de parada: fonte finita, limites do builder e pedido externo.
//!
//! ```text
//! cargo run --example stop_conditions
//! ```

use std::time::Duration;

use bevy_ecs::system::Local;
use perceptor::core::Result;
use perceptor::pipeline::despawn_processed_frames_system;
use perceptor::prelude::*;

/// Nome com que a fonte se identifica em `Sources`.
const SOURCE: &str = "contagem";
/// Quantos frames a fonte finita tem.
const FRAMES: u64 = 4;

fn frame(index: u64) -> Result<Frame> {
    Ok(Frame::new(
        FrameMeta::new(index, SOURCE),
        Image::<L8>::new(8, 8)?,
    ))
}

/// Fonte finita: `FRAMES` frames, depois avisa que terminou.
fn finite_source(
    mut commands: Commands,
    mut sources: ResMut<Sources>,
    mut sent: Local<u64>,
) -> Result<()> {
    if *sent == FRAMES {
        sources.finish(SOURCE);
        return Ok(());
    }
    commands.spawn(frame(*sent)?);
    *sent += 1;
    Ok(())
}

/// Fonte contínua: um frame por tick, para sempre. Não se registra em `Sources`.
fn endless_source(mut commands: Commands, mut sent: Local<u64>) -> Result<()> {
    commands.spawn(frame(*sent)?);
    *sent += 1;
    Ok(())
}

fn endless() -> PipelineBuilder {
    let mut builder = Pipeline::builder();
    builder.add_system(Stage::Input, fallible(endless_source));
    builder.add_system(Stage::Cleanup, despawn_processed_frames_system);
    builder
}

fn main() -> Result<()> {
    // 1. Fonte finita: `run()` termina sozinho, sem limite configurado.
    let mut builder = Pipeline::builder();
    builder
        .world_mut()
        .resource_mut::<Sources>()
        .register(SOURCE);
    builder.add_system(Stage::Input, fallible(finite_source));
    builder.add_system(Stage::Cleanup, despawn_processed_frames_system);
    let mut pipeline = builder.build()?;
    let reason = pipeline.run()?;
    println!("fonte finita:   {reason:?}");
    assert_eq!(reason, StopReason::EndOfStream);

    // 2. Limites sobre uma fonte contínua: vale o que for atingido primeiro.
    let mut pipeline = endless()
        .with_max_frames(10)
        .with_max_duration(Duration::from_secs(5))
        .build()?;
    let reason = pipeline.run()?;
    println!("limites:        {reason:?}");
    assert_eq!(reason, StopReason::FrameLimit);

    // 3. Um sistema decide parar.
    let mut builder = endless();
    builder.add_system(Stage::Output, |mut state: ResMut<PipelineState>| {
        if state.frame_count >= 3 {
            state.request_stop();
        }
    });
    let mut pipeline = builder.build()?;
    let reason = pipeline.run()?;
    println!("sistema:        {reason:?}");
    assert_eq!(reason, StopReason::Requested);

    // 4. Outra thread decide parar, por meio de um `StopHandle`.
    let mut pipeline = endless().build()?;
    let handle = pipeline.stop_handle();
    let stopper = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        handle.stop();
    });
    let reason = pipeline.run()?;
    stopper
        .join()
        .expect("a thread que pede parada não entra em pânico");
    let ticks = pipeline.world().resource::<PipelineState>().tick_count;
    println!("outra thread:   {reason:?} depois de {ticks} ticks");
    assert_eq!(reason, StopReason::Requested);
    Ok(())
}
