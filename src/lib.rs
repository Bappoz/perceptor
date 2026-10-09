//! # Perceptor
//!
//! Biblioteca de visão computacional em Rust: kernels puros e otimizados,
//! orquestrados por um pipeline ECS.
//!
//! Esta crate é a fachada do workspace e reexporta as camadas:
//!
//! | Módulo       | Crate                | Conteúdo                                   |
//! |--------------|----------------------|--------------------------------------------|
//! | [`core`]     | `perceptor-core`     | Tipos fundamentais                         |
//! | [`imgproc`]  | `perceptor-imgproc`  | Kernels de processamento como funções puras|
//! | [`pipeline`] | `perceptor-pipeline` | Pipeline ECS, stages e plugins             |
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

pub use perceptor_core as core;
pub use perceptor_imgproc as imgproc;
pub use perceptor_pipeline as pipeline;

/// Re-exports convenientes: `use perceptor::prelude::*;`.
pub mod prelude {
    pub use perceptor_pipeline::prelude::*;
}
