//! Sistema de escrita de frames em disco.

use std::path::Path;

use bevy_ecs::entity::Entity;
use bevy_ecs::system::{Query, Res, ResMut};
use perceptor_core::{Error, Result};
use tracing::info;

use crate::{
    errors::PipelineErrors,
    frame::Frame,
    pipeline::PipelineState,
    plugins::io::{config::ImageFormat, IoConfig},
};

/// Codifica um [`Frame`] e o grava em `path`.
///
/// Aceita frames de 1 (L), 3 (RGB) ou 4 (RGBA) canais.
///
/// # Errors
/// - [`Error::UnsupportedChannels`] para outros números de canais.
/// - [`Error::NonContiguous`] se o tensor não estiver em ordem de linha.
/// - [`Error::Encode`] se as dimensões excederem `u32` ou a gravação falhar.
pub fn write_frame(frame: &Frame, path: &Path, format: ImageFormat) -> Result<()> {
    let color = match frame.channels() {
        1 => image::ColorType::L8,
        3 => image::ColorType::Rgb8,
        4 => image::ColorType::Rgba8,
        channels => return Err(Error::UnsupportedChannels { channels }),
    };
    let raw = frame.data.as_slice().ok_or(Error::NonContiguous)?;
    let width = u32::try_from(frame.width()).map_err(|e| Error::encode(path, e))?;
    let height = u32::try_from(frame.height()).map_err(|e| Error::encode(path, e))?;
    let format = match format {
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
    };

    image::save_buffer_with_format(path, raw, width, height, color, format)
        .map_err(|e| Error::encode(path, e))
}

/// Sistema ECS: salva todos os frames presentes no world no caminho configurado.
/// Registrado em `Stage::Output` pelo [`IoPlugin`](crate::plugins::io::IoPlugin).
///
/// Uma falha de [`write_frame`] é reportada em [`PipelineErrors`] como erro
/// do frame correspondente.
pub fn image_writer_system(
    query: Query<(Entity, &Frame)>,
    config: Res<IoConfig>,
    mut state: ResMut<PipelineState>,
    mut errors: ResMut<PipelineErrors>,
) {
    for (entity, frame) in &query {
        match write_frame(frame, &config.output_path, config.format) {
            Ok(()) => info!("image_writer_system: salvo em {:?}", config.output_path),
            Err(e) => errors.report_frame("image_writer_system", entity, e),
        }
    }
    state.should_stop = true;
}
