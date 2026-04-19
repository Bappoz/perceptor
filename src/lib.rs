//! # Perceptor
//!
//! Biblioteca de Computer Vision baseada em ECS (Entity Component System).
//!
//! ## Conceito central
//!
//! Frames de vídeo são **Entidades** no mundo ECS. Transformações
//! (grayscale, edge detection, inferência ML) são **Sistemas** que operam
//! sobre **Componentes** dessas entidades. O [`Pipeline`] orquestra a
//! execução dessas transformações em ordem e (opcionalmente) em paralelo.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use perceptor::prelude::*;
//!
//! fn main() -> anyhow::Result<()> {
//!     let mut pipeline = Pipeline::builder()
//!         .add_plugin(IoPlugin::default())
//!         .add_plugin(FiltersPlugin::default())
//!         .build();
//!
//!     pipeline.run()
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod core;
pub mod plugins;
pub mod prelude;
pub mod tests;
