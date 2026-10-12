//! Sistema de leitura de frames a partir de arquivos de imagem.
//!
//! # Responsabilidade
//! Lê a imagem configurada no recurso [`IoConfig`], spawna uma Entidade
//! [`Frame`] no `World` e marca a fonte como terminada em [`Sources`].
//!
//! # Extensões futuras
//! - Suporte a vídeo via `ffmpeg-next` ou `opencv`
//! - Leitura de câmera via `nokhwa`
//! - Stream RTSP

use std::path::Path;

use bevy_ecs::prelude::*;
use perceptor_core::{Error, Result};
use tracing::info;

use crate::frame::{Frame, FrameMeta};
use crate::plugins::io::IoConfig;
use crate::stop::Sources;

/// Nome com que o leitor de imagem se registra em [`Sources`].
pub const IMAGE_READER_SOURCE: &str = "image_reader";

/// Lê e decodifica uma imagem do disco como [`Frame`] RGB.
///
/// # Errors
/// - [`Error::Decode`] se o arquivo não existir, não puder ser lido ou não for
///   uma imagem em formato suportado.
/// - [`Error::Allocation`] se o buffer do frame não puder ser alocado.
pub fn read_frame(path: &Path, index: u64) -> Result<Frame> {
    let img = image::open(path).map_err(|e| Error::decode(path, e))?;
    let meta = FrameMeta::new(index, path.to_string_lossy());
    Frame::from_dynamic_image(meta, img)
}

/// Sistema ECS: lê o arquivo configurado e o spawna como entidade, uma única
/// vez. Registrado em `Stage::Input` pelo
/// [`IoPlugin`](crate::plugins::io::IoPlugin), por meio de
/// [`fallible`](crate::fallible).
///
/// A fonte é um único arquivo: depois da primeira tentativa, com sucesso ou
/// não, ela consta como terminada em [`Sources`] e o sistema não faz mais
/// nada. É o que encerra `run()` com
/// [`StopReason::EndOfStream`](crate::StopReason::EndOfStream).
///
/// # Errors
/// O erro de [`read_frame`].
pub fn image_reader_system(
    mut commands: Commands,
    mut config: ResMut<IoConfig>,
    mut sources: ResMut<Sources>,
) -> Result<()> {
    if sources.is_finished(IMAGE_READER_SOURCE) {
        return Ok(());
    }
    sources.finish(IMAGE_READER_SOURCE);
    let frame = read_frame(&config.input_path, config.next_index)?;

    info!(
        index = config.next_index,
        path = ?config.input_path,
        "image_reader_system: spawned frame {}x{}x{}",
        frame.height(),
        frame.width(),
        frame.channels()
    );

    commands.spawn(frame);
    config.next_index += 1;
    Ok(())
}
