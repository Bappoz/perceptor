//! Plugin de I/O: leitura de imagens/vídeos e escrita de resultados.
//!
//! # Sistemas registrados
//! - [`image_reader`] → `InputStage`
//! - [`image_writer`] → `OutputStage`

pub mod config;
pub mod image_reader;
pub mod image_writer;

use std::path::PathBuf;

use bevy_ecs::prelude::*;

use crate::{
    core::{pipeline::PipelineBuilder, plugin::Plugin},
    plugins::io::{config::ImageFormat, image_writer::image_writer_system},
};
use image_reader::image_reader_system;

/// Recurso ECS que configura a fonte e o destino de I/O para o [`IoPlugin`].
#[derive(Resource, Debug, Default)]
pub struct IoConfig {
    /// Caminho do arquivo de imagem/vídeo de entrada.
    pub input_path: PathBuf,
    /// Caminho de destino para o frame de saída.
    pub output_path: PathBuf,
    /// Formato de saída (PNG ou JPEG).
    pub format: ImageFormat,
    /// Índice do próximo frame a ser lido (gerenciado internamente).
    pub(crate) next_index: u64,
}

/// Plugin que registra sistemas de leitura e escrita de frames.
#[derive(Debug, Default)]
pub struct IoPlugin {
    /// Caminho ou URL da fonte de frames (arquivo, câmera device, RTSP…).
    pub input_path: PathBuf,
    /// Caminho de destino para salvar os frames processados.
    pub output_path: PathBuf,
    /// Formato de saída.
    pub format: ImageFormat,
}

impl Plugin for IoPlugin {
    fn name(&self) -> &str {
        "IoPlugin"
    }

    fn build(&self, builder: &mut PipelineBuilder) {
        builder.add_input_system(image_reader_system);
        builder.add_output_system(image_writer_system);
    }

    fn finish(&self, builder: &mut PipelineBuilder) {
        builder.world_mut().insert_resource(IoConfig {
            input_path: self.input_path.clone(),
            output_path: self.output_path.clone(),
            format: self.format.clone(),
            next_index: 0,
        });
    }
}
