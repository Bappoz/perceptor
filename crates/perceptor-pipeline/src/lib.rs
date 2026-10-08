//! # perceptor-pipeline
//!
//! Camada de orquestração do Perceptor sobre ECS (`bevy_ecs`).
//!
//! Frames são **Entidades**, transformações são **Sistemas** e os resultados
//! de cada etapa são **Componentes**. O [`Pipeline`] executa os stages em
//! ordem a cada tick; [`Plugin`]s registram sistemas nos stages corretos.
//!
//! Os algoritmos em si vivem em `perceptor-imgproc` como funções puras —
//! esta crate apenas os conecta ao `World`.

#![allow(clippy::module_name_repetitions)]
// Parâmetros de sistema ECS (`Query`, `Res`, `Commands`) são sempre recebidos por valor
// e filtros de query são tuplas aninhadas por construção.
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)]

pub mod frame;
pub mod pipeline;
pub mod plugin;
pub mod plugins;
pub mod prelude;
pub mod schedule;

pub use frame::{Frame, FrameMeta};
pub use pipeline::{Pipeline, PipelineBuilder, PipelineState};
pub use plugin::Plugin;
