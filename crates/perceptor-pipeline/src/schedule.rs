//! Stages: as fases de um tick do pipeline.
//!
//! Todos os sistemas vivem em um único `Schedule`; cada [`Stage`] é um
//! `SystemSet`, e os sets são encadeados na ordem canônica:
//!
//! ```text
//! Input → PreProcess → Process → PostProcess → Output → Cleanup
//! ```
//!
//! Entre dois stages há um ponto de sincronização: os `Commands` emitidos em
//! um stage já estão aplicados quando o seguinte começa. Dentro de um stage a
//! ordem é indefinida, a menos que os sistemas sejam ordenados com
//! `.chain()`, `.before()` ou `.after()`. Os sistemas rodam um de cada vez,
//! na thread que chamou `tick()`; o paralelismo fica dentro dos kernels.

use bevy_ecs::schedule::SystemSet;

/// Fase de execução de um tick.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Leitura de fontes (câmera, arquivo, rede): sistemas criam entidades `Frame`.
    Input,
    /// Pré-processamento (resize, normalização, crop).
    PreProcess,
    /// Processamento principal (filtros, detecção, segmentação).
    Process,
    /// Pós-processamento (NMS, tracking, agregação de resultados).
    PostProcess,
    /// Saída (exibição, escrita em disco, envio via rede).
    Output,
    /// Limpeza: remoção das entidades que a saída já consumiu.
    Cleanup,
}

impl Stage {
    /// Todos os stages, na ordem em que rodam.
    pub const ALL: [Self; 6] = [
        Self::Input,
        Self::PreProcess,
        Self::Process,
        Self::PostProcess,
        Self::Output,
        Self::Cleanup,
    ];
}
