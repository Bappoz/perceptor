//! Sistema de leitura de frames a partir de arquivos de imagem.
//!
//! # Responsabilidade
//! Lê a fonte configurada no recurso [`IoConfig`] e spawna uma Entidade
//! [`Frame`] no `World` a cada tick.
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

/// Lê e decodifica uma imagem do disco como [`Frame`] RGB.
///
/// # Errors
/// [`Error::Decode`] se o arquivo não existir, não puder ser lido ou não for
/// uma imagem em formato suportado.
pub fn read_frame(path: &Path, index: u64) -> Result<Frame> {
    let img = image::open(path).map_err(|e| Error::decode(path, e))?;
    let meta = FrameMeta {
        index,
        timestamp_us: 0,
        source: path.to_string_lossy().into_owned(),
    };
    Ok(Frame::from_dynamic_image(meta, img))
}

/// Sistema ECS: lê um frame da fonte configurada e o spawna como entidade.
/// Registrado em `Stage::Input` pelo [`IoPlugin`](crate::plugins::io::IoPlugin),
/// por meio de [`fallible`](crate::fallible).
///
/// # Errors
/// O erro de [`read_frame`]. Como a fonte é um único arquivo, a falha também
/// sinaliza parada em [`PipelineState`](crate::pipeline::PipelineState): sem
/// isso, em uma política que não interrompe, o pipeline falharia a cada tick.
pub fn image_reader_system(
    mut commands: Commands,
    mut config: ResMut<IoConfig>,
    mut state: ResMut<crate::pipeline::PipelineState>,
) -> Result<()> {
    let frame = read_frame(&config.input_path, config.next_index).inspect_err(|_| {
        state.should_stop = true;
    })?;

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
