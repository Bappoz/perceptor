//! Ctrl-C como condição de parada.
//!
//! Em binário de teste próprio: o tratador de sinal é do processo inteiro, e
//! o SIGINT enviado aqui mataria qualquer outro teste que não o instalasse.
#![cfg(all(unix, feature = "ctrlc"))]

use std::process::Command;
use std::time::Duration;

use bevy_ecs::prelude::*;
use perceptor_pipeline::{Pipeline, PipelineState, Stage, StopReason};

/// Rede de segurança: sem o tratador, o teste falha em `TickLimit` em vez de travar.
const SAFETY_TICKS: u64 = 5_000;

fn send_sigint_to_self() {
    let status = Command::new("kill")
        .args(["-INT", &std::process::id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn ctrl_c_stops_the_run_and_later_pipelines_start_clean() {
    let mut builder = Pipeline::builder()
        .with_max_ticks(SAFETY_TICKS)
        .stop_on_ctrl_c();
    builder.add_system(Stage::Process, |state: Res<PipelineState>| {
        if state.tick_count == 2 {
            send_sigint_to_self();
        }
        // O tratador roda em outra thread: dá tempo de ele registrar o sinal.
        std::thread::sleep(Duration::from_millis(1));
    });
    let mut pipeline = builder.build().unwrap();

    assert_eq!(pipeline.run().unwrap(), StopReason::Interrupted);
    assert!(pipeline.world().resource::<PipelineState>().tick_count >= 3);

    // O Ctrl-C anterior não vale para um pipeline construído depois dele.
    let mut later = Pipeline::builder()
        .with_max_ticks(3)
        .stop_on_ctrl_c()
        .build()
        .unwrap();
    assert_eq!(later.run().unwrap(), StopReason::TickLimit);
}
