//! Métricas do pipeline: onde o tempo de cada tick é gasto.
//!
//! Disponível com a feature `metrics` (ligada por padrão). Com ela, o
//! pipeline registra um sistema marcador em cada fronteira entre stages; cada
//! marcador lê o relógio uma vez, fecha o span do stage que terminou e abre o
//! do seguinte. Sem a feature nada disso é compilado.
//!
//! Os spans `tracing` são `tick` (campo `tick`) e, dentro dele, `stage`
//! (campo `stage`), ambos no nível `DEBUG`. Qualquer span criado por um
//! sistema fica aninhado no span do seu stage.

use std::time::{Duration, Instant};

use bevy_ecs::prelude::*;
use tracing::Span;

use crate::frame::Frame;
use crate::schedule::Stage;

/// Estatísticas de duração de um stage, ou do tick inteiro.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Timing {
    /// Duração na execução mais recente.
    pub last: Duration,
    /// Maior duração observada.
    pub max: Duration,
    /// Soma de todas as durações.
    pub total: Duration,
    /// Número de execuções medidas.
    pub runs: u64,
}

impl Timing {
    /// Duração média, ou zero se nada foi medido.
    #[must_use]
    pub fn mean(&self) -> Duration {
        // `runs` só passa de `u32::MAX` depois de bilhões de ticks; a média
        // de então em diante usa o teto, o que a superestima em vez de falhar.
        match u32::try_from(self.runs) {
            Ok(0) => Duration::ZERO,
            Ok(runs) => self.total / runs,
            Err(_) => self.total / u32::MAX,
        }
    }

    fn record(&mut self, elapsed: Duration) {
        self.last = elapsed;
        self.max = self.max.max(elapsed);
        self.total += elapsed;
        self.runs += 1;
    }
}

/// Recurso com as métricas acumuladas desde a criação do pipeline.
///
/// Leia com [`Pipeline::metrics`](crate::Pipeline::metrics), ou como
/// `Res<PipelineMetrics>` dentro de um sistema.
#[derive(Resource, Debug, Default)]
pub struct PipelineMetrics {
    stages: [Timing; Stage::ALL.len()],
    tick: Timing,
    frames_processed: u64,
    frames_dropped: u64,
    first_tick_start: Option<Instant>,
    last_tick_end: Option<Instant>,
    /// Stage em andamento: índice, início e span aberto.
    open: Option<(usize, Instant, Span)>,
}

impl PipelineMetrics {
    /// Duração de `stage`: sistemas do stage, aplicação dos `Commands` deles
    /// e, em `ErrorPolicy::SkipFrame`, o descarte dos frames com erro.
    #[must_use]
    pub fn stage(&self, stage: Stage) -> Timing {
        self.stages[stage_index(stage)]
    }

    /// Duração do tick inteiro (todos os stages mais o custo do schedule).
    #[must_use]
    pub fn tick(&self) -> Timing {
        self.tick
    }

    /// Frames que chegaram ao início de [`Stage::Output`], somados por tick.
    ///
    /// Um frame que permanece no `World` por vários ticks é contado em cada um.
    #[must_use]
    pub fn frames_processed(&self) -> u64 {
        self.frames_processed
    }

    /// Frames removidos por `ErrorPolicy::SkipFrame`.
    #[must_use]
    pub fn frames_dropped(&self) -> u64 {
        self.frames_dropped
    }

    /// Frames processados por segundo de relógio, do início do primeiro tick
    /// ao fim do último — inclui o tempo ocioso entre ticks.
    ///
    /// `None` antes do primeiro tick.
    #[must_use]
    pub fn fps(&self) -> Option<f64> {
        let elapsed = self
            .last_tick_end?
            .duration_since(self.first_tick_start?)
            .as_secs_f64();
        // Perda de precisão só acima de 2⁵³ frames.
        #[allow(clippy::cast_precision_loss)]
        let frames = self.frames_processed as f64;
        (elapsed > 0.0).then(|| frames / elapsed)
    }

    pub(crate) fn begin_tick(&mut self) -> Instant {
        let now = Instant::now();
        self.first_tick_start.get_or_insert(now);
        now
    }

    pub(crate) fn end_tick(&mut self, started: Instant, dropped: u64) {
        let now = Instant::now();
        self.tick.record(now.duration_since(started));
        self.last_tick_end = Some(now);
        self.frames_dropped += dropped;
    }

    /// Fronteira `boundary`: fecha o stage `boundary - 1` e abre o `boundary`.
    fn cross(&mut self, boundary: usize) {
        let now = Instant::now();
        if let Some((index, started, span)) = self.open.take() {
            exit(&span);
            self.stages[index].record(now.duration_since(started));
        }
        if let Some(stage) = Stage::ALL.get(boundary) {
            let span = tracing::debug_span!("stage", stage = ?stage);
            enter(&span);
            self.open = Some((boundary, now, span));
        }
    }
}

fn stage_index(stage: Stage) -> usize {
    Stage::ALL
        .iter()
        .position(|s| *s == stage)
        .unwrap_or_default()
}

// Um `EnteredSpan` não é `Send`, então não pode viver em um recurso. Entrar e
// sair pelo subscriber é o que a guarda faria; é correto aqui porque o
// pipeline usa o executor single-thread: todos os marcadores rodam na thread
// que chamou `tick()`, na ordem das fronteiras.
fn enter(span: &Span) {
    span.with_subscriber(|(id, dispatch)| dispatch.enter(id));
}

fn exit(span: &Span) {
    span.with_subscriber(|(id, dispatch)| dispatch.exit(id));
}

/// Set do marcador da fronteira `n`: depois do stage `n - 1`, antes do `n`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Boundary(pub usize);

/// Registra os marcadores nas `Stage::ALL.len() + 1` fronteiras do tick.
pub(crate) fn add_boundary_markers(schedule: &mut Schedule) {
    let output = stage_index(Stage::Output);
    for boundary in 0..=Stage::ALL.len() {
        let marker = move |mut metrics: ResMut<PipelineMetrics>, frames: Query<(), With<Frame>>| {
            if boundary == output {
                metrics.frames_processed += frames.iter().count() as u64;
            }
            metrics.cross(boundary);
        };
        let mut config = marker.in_set(Boundary(boundary));
        if let Some(previous) = boundary.checked_sub(1) {
            config = config.after(Stage::ALL[previous]);
        }
        if let Some(next) = Stage::ALL.get(boundary) {
            config = config.before(*next);
        }
        schedule.add_systems(config);
    }
}
