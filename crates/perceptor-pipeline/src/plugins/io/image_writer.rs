//! Sistema de escrita de frames em disco.

use std::path::Path;

use bevy_ecs::entity::Entity;
use bevy_ecs::system::{Query, Res, ResMut};
use perceptor_core::{Error, PixelFormat, Result};
use tracing::info;

use crate::{
    errors::PipelineErrors,
    frame::Frame,
    plugins::io::{config::ImageFormat, IoConfig},
};

/// Codifica um [`Frame`] e o grava em `path`.
///
/// Aceita frames em `L8`, `La8`, `Rgb8`, `Rgba8` e `L16`.
///
/// # Errors
/// - [`Error::UnsupportedFormat`] para os demais formatos (`Bgr8` e ponto
///   flutuante): nenhum deles é convertido implicitamente.
/// - [`Error::Encode`] se as dimensões excederem `u32`, se o formato de
///   arquivo não comportar o formato de pixel (ex.: `L16` em JPEG) ou se a
///   gravação falhar.
pub fn write_frame(frame: &Frame, path: &Path, format: ImageFormat) -> Result<()> {
    let color = match frame.format() {
        PixelFormat::L8 => image::ColorType::L8,
        PixelFormat::La8 => image::ColorType::La8,
        PixelFormat::Rgb8 => image::ColorType::Rgb8,
        PixelFormat::Rgba8 => image::ColorType::Rgba8,
        PixelFormat::L16 => image::ColorType::L16,
        format => return Err(Error::UnsupportedFormat { format }),
    };
    // `image` espera os subpixels de 16 bits na ordem de bytes nativa.
    let raw = frame.data.as_bytes();
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
    mut errors: ResMut<PipelineErrors>,
) {
    for (entity, frame) in &query {
        match write_frame(frame, &config.output_path, config.format) {
            Ok(()) => info!("image_writer_system: salvo em {:?}", config.output_path),
            Err(e) => errors.report_frame("image_writer_system", entity, e),
        }
    }
}
