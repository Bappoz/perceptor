//! Plugin de I/O: leitura de imagens/vídeos e escrita de resultados.
//!
//! # Sistemas registrados
//! - [`video_reader`] → `InputStage`
//! - `image_writer`   → `OutputStage` (TODO)

pub mod video_reader;

use crate::core::{pipeline::PipelineBuilder, plugin::Plugin};
use video_reader::image_reader_system;

/// Plugin que registra sistemas de leitura e escrita de frames.
///
/// # Configuração
/// ```rust,no_run
/// use perceptor::prelude::*;
///
/// let pipeline = Pipeline::builder()
///     .add_plugin(IoPlugin { source: "input.jpg".into(), ..Default::default() })
///     .build();
/// ```
#[derive(Debug, Default)]
pub struct IoPlugin {
    /// Caminho ou URL da fonte de frames (arquivo, câmera device, RTSP…).
    pub source: String,
}

impl Plugin for IoPlugin {
    fn name(&self) -> &str { "IoPlugin" }

    fn build(&self, builder: &mut PipelineBuilder) {
        builder.add_input_system(image_reader_system);
        // TODO: builder.add_output_system(image_writer_system);
    }
}
