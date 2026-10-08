//! Labels de schedule que definem as fases de execução do pipeline.
//!
//! A ordem canônica de execução é:
//!
//! ```text
//! InputStage → PreProcessStage → ProcessStage → PostProcessStage → OutputStage
//! ```
//!
//! Sistemas são registrados em um desses labels. O [`Pipeline`] executa
//! os schedules nessa ordem a cada tick.

use bevy_ecs::schedule::ScheduleLabel;

/// Fase 1 — Leitura de fontes (câmera, arquivo, rede).
/// Sistemas aqui produzem Entidades `Frame` no `World`.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct InputStage;

/// Fase 2 — Pré-processamento (resize, normalização, crop).
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreProcessStage;

/// Fase 3 — Processamento principal (filtros, detecção, segmentação).
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProcessStage;

/// Fase 4 — Pós-processamento (NMS, tracking, agregação de resultados).
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PostProcessStage;

/// Fase 5 — Saída (exibição, escrita em disco, envio via rede).
/// Sistemas aqui consomem/destroem Entidades `Frame` processadas.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutputStage;

/// Ordem canônica dos stages para uso interno do `Pipeline`.
pub const STAGE_ORDER: &[&str] = &[
    "InputStage",
    "PreProcessStage",
    "ProcessStage",
    "PostProcessStage",
    "OutputStage",
];
