//! # perceptor-pipeline
//!
//! Camada de orquestração do Perceptor sobre ECS (`bevy_ecs`).
//!
//! Frames são **Entidades**, transformações são **Sistemas** e os resultados
//! de cada etapa são **Componentes**. O [`Pipeline`] executa os stages em
//! ordem a cada tick; [`Plugin`]s registram sistemas no [`Stage`] correto.
//!
//! Os algoritmos em si vivem em `perceptor-imgproc` como funções puras —
//! esta crate apenas os conecta ao `World`.

#![allow(clippy::module_name_repetitions)]
// Parâmetros de sistema ECS (`Query`, `Res`, `Commands`) são sempre recebidos por valor
// e filtros de query são tuplas aninhadas por construção.
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)]

pub mod errors;
pub mod frame;
#[cfg(feature = "metrics")]
pub mod metrics;
pub mod pipeline;
pub mod plugin;
pub mod plugins;
pub mod prelude;
pub mod schedule;
pub mod stop;
pub mod testing;

pub use errors::{fallible, ErrorPolicy, PipelineErrors, SystemError};
pub use frame::{Frame, FrameData, FrameMeta, FramePixel};
#[cfg(feature = "metrics")]
pub use metrics::{PipelineMetrics, Timing};
pub use pipeline::{despawn_processed_frames_system, Pipeline, PipelineBuilder, PipelineState};
pub use plugin::Plugin;
pub use schedule::Stage;
pub use stop::{Sources, StopHandle, StopReason};
