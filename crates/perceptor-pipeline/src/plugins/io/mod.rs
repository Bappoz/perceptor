//! Plugin de I/O: leitura de imagens/vídeos e escrita de resultados.
//!
//! # Sistemas registrados
//! - [`image_reader`] → `Stage::Input`
//! - [`image_writer`] → `Stage::Output`
//! - [`despawn_processed_frames_system`] → `Stage::Cleanup` (a menos que `keep_frames`)

pub mod config;
pub mod image_reader;
pub mod image_writer;

use std::path::PathBuf;

use bevy_ecs::prelude::*;

use crate::{
    errors::fallible,
    pipeline::{despawn_processed_frames_system, PipelineBuilder},
    plugin::Plugin,
    plugins::io::{config::ImageFormat, image_writer::image_writer_system},
};
use image_reader::{image_reader_system, IMAGE_READER_SOURCE};

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
    /// Mantém os frames no `World` depois da saída, para inspeção.
    ///
    /// Por padrão o frame é removido em `Stage::Cleanup` do tick em que foi
    /// escrito; com `true` ele continua no `World`.
    pub keep_frames: bool,
}

impl Plugin for IoPlugin {
    fn name(&self) -> &'static str {
        "IoPlugin"
    }

    fn build(&self, builder: &mut PipelineBuilder) {
        builder
            .world_mut()
            .resource_mut::<crate::stop::Sources>()
            .register(IMAGE_READER_SOURCE);
        builder.add_input_system(fallible(image_reader_system));
        builder.add_output_system(image_writer_system);
        if !self.keep_frames {
            builder.add_cleanup_system(despawn_processed_frames_system);
        }
    }

    fn finish(&self, builder: &mut PipelineBuilder) {
        builder.world_mut().insert_resource(IoConfig {
            input_path: self.input_path.clone(),
            output_path: self.output_path.clone(),
            format: self.format,
            next_index: 0,
        });
    }
}
