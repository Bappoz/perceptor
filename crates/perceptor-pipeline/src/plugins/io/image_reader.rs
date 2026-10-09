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
use tracing::{info, warn};

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
/// Registrado no `InputStage` pelo [`IoPlugin`](crate::plugins::io::IoPlugin).
pub fn image_reader_system(
    mut commands: Commands,
    mut config: ResMut<IoConfig>,
    mut state: ResMut<crate::pipeline::PipelineState>,
) {
    if config.input_path.as_os_str().is_empty() {
        warn!("image_reader_system: IoConfig.source não configurado, pulando tick");
        state.should_stop = true;
        return;
    }

    let frame = match read_frame(&config.input_path, config.next_index) {
        Ok(frame) => frame,
        Err(e) => {
            warn!("image_reader_system: {e}");
            state.should_stop = true;
            return;
        }
    };

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
}
