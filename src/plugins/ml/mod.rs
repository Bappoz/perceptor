//! Plugin de inferência ML.
//!
//! # Backends planejados
//! | Feature flag | Crate        | Backend              |
//! |--------------|--------------|----------------------|
//! | `ml`         | `ort`        | ONNX Runtime         |
//! | `ml`         | `tch-rs`     | PyTorch/LibTorch     |
//! | `gpu`        | `wgpu`       | WebGPU compute       |
//!
//! # Design
//! O [`InferenceSystem`] recebe um `Model` como recurso e itera sobre
//! frames com [`SobelTag`] (ou qualquer predicado), rodando inferência
//! e anexando um componente [`Prediction`] à entidade.

pub mod inference;

use crate::core::{pipeline::PipelineBuilder, plugin::Plugin};
use inference::inference_system;

/// Plugin de Machine Learning / inferência de modelos.
///
/// Requer feature `ml` para backends reais. Sem a feature, os sistemas
/// são registrados mas funcionam como no-ops com aviso de log.
#[derive(Debug, Default)]
pub struct MlPlugin {
    /// Caminho para o arquivo de modelo (`.onnx`, `.pt`, etc.).
    pub model_path: String,
}

impl Plugin for MlPlugin {
    fn name(&self) -> &str { "MlPlugin" }

    fn build(&self, builder: &mut PipelineBuilder) {
        builder.add_post_process_system(inference_system);
    }
}
