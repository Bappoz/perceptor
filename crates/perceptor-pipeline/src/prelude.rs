//! Re-exports convenientes para uso público da biblioteca.
//!
//! Importe tudo com `use perceptor::prelude::*;`

// Core
pub use crate::{
    errors::{fallible, ErrorPolicy, PipelineErrors},
    frame::{Frame, FrameData, FrameMeta},
    ops::Op,
    pipeline::{Pipeline, PipelineBuilder, PipelineState},
    plugin::Plugin,
    schedule::Stage,
    stop::{Sources, StopHandle, StopReason},
};

#[cfg(feature = "metrics")]
pub use crate::metrics::PipelineMetrics;

// Plugins prontos para uso
pub use crate::plugins::{filters::FiltersPlugin, io::IoPlugin, ml::MlPlugin};

// Re-exports de crates externas frequentemente necessários
pub use bevy_ecs::prelude::{
    Commands, Component, Entity, Query, Res, ResMut, Resource, With, Without, World,
};
pub use perceptor_core::{
    Bgr8, Error, Hsl8, HslF32, Hsv8, HsvF32, Image, ImageView, ImageViewMut, La8, Pixel,
    PixelFormat, Result, Rgb8, RgbF32, Rgba8, L16, L8, LF32,
};
