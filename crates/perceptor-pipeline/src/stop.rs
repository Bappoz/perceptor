//! Condições de parada: o que encerra [`Pipeline::run`](crate::Pipeline::run).
//!
//! Ao fim de cada tick o pipeline verifica, nesta ordem, se:
//!
//! 1. um sistema pediu parada com [`PipelineState::request_stop`](crate::PipelineState::request_stop);
//! 2. alguém fora do pipeline chamou [`StopHandle::stop`];
//! 3. o processo recebeu Ctrl-C (feature `ctrlc`);
//! 4. todas as fontes registradas em [`Sources`] terminaram;
//! 5. um limite do builder foi atingido: frames, ticks ou duração.
//!
//! A primeira que valer vira o [`StopReason`] devolvido por `run()`. A
//! verificação é sempre no fim do tick: nenhum tick é interrompido no meio,
//! e todo frame que entrou em `Stage::Input` passa pelos stages seguintes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bevy_ecs::prelude::*;

use crate::frame::Frame;
use crate::pipeline::PipelineState;

/// Por que o pipeline parou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StopReason {
    /// Um sistema chamou [`PipelineState::request_stop`](crate::PipelineState::request_stop),
    /// ou alguém chamou [`StopHandle::stop`].
    Requested,
    /// O processo recebeu Ctrl-C (feature `ctrlc`).
    Interrupted,
    /// Todas as fontes registradas em [`Sources`] terminaram.
    EndOfStream,
    /// O limite de [`with_max_frames`](crate::PipelineBuilder::with_max_frames) foi atingido.
    FrameLimit,
    /// O limite de [`with_max_ticks`](crate::PipelineBuilder::with_max_ticks) foi atingido.
    TickLimit,
    /// O limite de [`with_max_duration`](crate::PipelineBuilder::with_max_duration) foi atingido.
    TimeLimit,
}

/// Limites configurados no builder.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct StopLimits {
    pub(crate) ticks: Option<u64>,
    pub(crate) frames: Option<u64>,
    pub(crate) duration: Option<Duration>,
}

impl StopLimits {
    /// O primeiro limite atingido, na ordem frames → ticks → duração.
    pub(crate) fn reached(&self, state: &PipelineState, elapsed: Duration) -> Option<StopReason> {
        if self.frames.is_some_and(|max| state.frame_count >= max) {
            Some(StopReason::FrameLimit)
        } else if self.ticks.is_some_and(|max| state.tick_count >= max) {
            Some(StopReason::TickLimit)
        } else if self.duration.is_some_and(|max| elapsed >= max) {
            Some(StopReason::TimeLimit)
        } else {
            None
        }
    }
}

/// Pedido de parada vindo de fora do pipeline, de qualquer thread.
///
/// Obtido com [`Pipeline::stop_handle`](crate::Pipeline::stop_handle); os
/// clones compartilham o mesmo estado. O pipeline só o consulta no fim de
/// cada tick, então o tick em andamento termina antes de `run()` voltar.
#[derive(Debug, Clone, Default)]
pub struct StopHandle(Arc<AtomicBool>);

impl StopHandle {
    /// Pede que o pipeline pare ao fim do tick em andamento.
    pub fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// `true` depois de [`stop`](Self::stop).
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Recurso em que as fontes de frames avisam que terminaram.
///
/// Uma fonte finita (arquivo, sequência, vídeo) se registra com
/// [`register`](Self::register) ao ser adicionada ao pipeline e chama
/// [`finish`](Self::finish) quando não tem mais frames. Quando todas as
/// fontes registradas terminaram, `run()` encerra com
/// [`StopReason::EndOfStream`]. Sem nenhuma fonte registrada essa condição
/// nunca vale: fontes contínuas (câmera, sintética) simplesmente não se
/// registram.
#[derive(Resource, Debug, Default)]
pub struct Sources {
    /// `(nome, terminou)`, na ordem de registro.
    entries: Vec<(&'static str, bool)>,
}

impl Sources {
    /// Registra a fonte `name` como ativa. Registrar de novo um nome já
    /// conhecido o reabre.
    pub fn register(&mut self, name: &'static str) {
        match self.entry(name) {
            Some(finished) => *finished = false,
            None => self.entries.push((name, false)),
        }
    }

    /// Marca a fonte `name` como terminada. Uma fonte que não se registrou
    /// passa a constar, já terminada.
    pub fn finish(&mut self, name: &'static str) {
        match self.entry(name) {
            Some(finished) => *finished = true,
            None => self.entries.push((name, true)),
        }
    }

    /// `true` se `name` foi registrada e já terminou.
    #[must_use]
    pub fn is_finished(&self, name: &str) -> bool {
        self.entries.iter().any(|(n, done)| *n == name && *done)
    }

    /// `true` se há ao menos uma fonte e todas terminaram.
    #[must_use]
    pub fn all_finished(&self) -> bool {
        !self.entries.is_empty() && self.entries.iter().all(|(_, done)| *done)
    }

    /// Número de fontes registradas.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` se nenhuma fonte foi registrada.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn entry(&mut self, name: &str) -> Option<&mut bool> {
        self.entries
            .iter_mut()
            .find_map(|(n, done)| (*n == name).then_some(done))
    }
}

/// Soma em [`PipelineState::frame_count`] os frames criados desde a última
/// execução. Roda logo depois de `Stage::Input`.
pub(crate) fn count_new_frames(new: Query<(), Added<Frame>>, mut state: ResMut<PipelineState>) {
    let added = new.iter().count() as u64;
    if added > 0 {
        state.frame_count += added;
    }
}

/// Ctrl-C como condição de parada, para o processo inteiro.
#[cfg(feature = "ctrlc")]
pub(crate) mod ctrl_c {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::OnceLock;

    use perceptor_core::{Error, Result};

    /// Quantos Ctrl-C o processo já recebeu. Um contador, e não uma flag,
    /// para que um pipeline criado depois de um Ctrl-C não nasça parado.
    static RECEIVED: AtomicU64 = AtomicU64::new(0);
    /// Resultado da única instalação do tratador neste processo.
    static INSTALLED: OnceLock<std::result::Result<(), String>> = OnceLock::new();

    /// Instala o tratador (uma vez por processo) e devolve a contagem atual.
    pub(crate) fn watch() -> Result<u64> {
        INSTALLED
            .get_or_init(|| {
                ctrlc::set_handler(|| {
                    RECEIVED.fetch_add(1, Ordering::Relaxed);
                })
                .map_err(|e| e.to_string())
            })
            .clone()
            .map_err(|message| Error::SignalHandler {
                source: message.into(),
            })?;
        Ok(received())
    }

    pub(crate) fn received() -> u64 {
        RECEIVED.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_start_empty_and_not_finished() {
        let sources = Sources::default();
        assert!(sources.is_empty());
        assert!(!sources.all_finished());
        assert!(!sources.is_finished("x"));
    }

    #[test]
    fn all_finished_requires_every_source() {
        let mut sources = Sources::default();
        sources.register("a");
        sources.register("b");
        sources.finish("a");
        assert!(sources.is_finished("a"));
        assert!(!sources.all_finished());
        sources.finish("b");
        assert!(sources.all_finished());
        assert_eq!(sources.len(), 2);
    }

    #[test]
    fn registering_again_reopens_a_source() {
        let mut sources = Sources::default();
        sources.finish("a");
        assert!(sources.all_finished());
        sources.register("a");
        assert!(!sources.all_finished());
        assert_eq!(sources.len(), 1);
    }

    #[test]
    fn stop_handle_clones_share_state() {
        let handle = StopHandle::default();
        let clone = handle.clone();
        clone.stop();
        assert!(handle.is_stopped());
    }

    #[test]
    fn limits_report_the_first_one_reached() {
        let limits = StopLimits {
            ticks: Some(2),
            frames: Some(2),
            duration: Some(Duration::from_secs(1)),
        };
        let mut state = PipelineState::default();
        assert_eq!(limits.reached(&state, Duration::ZERO), None);
        assert_eq!(
            limits.reached(&state, Duration::from_secs(1)),
            Some(StopReason::TimeLimit)
        );
        state.tick_count = 2;
        assert_eq!(
            limits.reached(&state, Duration::from_secs(1)),
            Some(StopReason::TickLimit)
        );
        state.frame_count = 2;
        assert_eq!(
            limits.reached(&state, Duration::ZERO),
            Some(StopReason::FrameLimit)
        );
    }
}
