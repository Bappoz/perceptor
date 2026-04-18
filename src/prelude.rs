//! Re-exports convenientes para uso público da biblioteca.
//!
//! Importe tudo com `use perceptor::prelude::*;`

// Core
pub use crate::core::{
    frame::Frame,
    pipeline::{Pipeline, PipelineBuilder},
    schedule::{InputStage, OutputStage, ProcessStage},
    plugin::Plugin,
};

// Plugins prontos para uso
pub use crate::plugins::{
    filters::FiltersPlugin,
    io::IoPlugin,
    ml::MlPlugin,
};

// Re-exports de crates externas frequentemente necessários
pub use anyhow::{Context, Result};
pub use bevy_ecs::prelude::{Commands, Component, Entity, Query, Res, ResMut, Resource, World};
pub use ndarray::Array3;
