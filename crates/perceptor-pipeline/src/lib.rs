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

pub mod frame;
pub mod pipeline;
pub mod plugin;
pub mod plugins;
pub mod prelude;
pub mod schedule;
pub mod tests;

pub use frame::{Frame, FrameMeta};
pub use pipeline::{Pipeline, PipelineBuilder, PipelineState};
pub use plugin::Plugin;
