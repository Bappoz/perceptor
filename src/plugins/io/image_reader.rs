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

use bevy_ecs::prelude::*;
use tracing::{info, warn};

use crate::core::frame::{Frame, FrameMeta};
use crate::plugins::io::IoConfig;

/// Sistema ECS: lê um frame da fonte configurada e o spawna como entidade.
/// Registrado no `InputStage` pelo [`IoPlugin`].
pub fn image_reader_system(
    mut commands: Commands,
    mut config: ResMut<IoConfig>,
    mut state: ResMut<crate::core::pipeline::PipelineState>,
) {
    if config.input_path.as_os_str().is_empty() {
        warn!("image_reader_system: IoConfig.source não configurado, pulando tick");
        state.should_stop = true;
        return;
    }

    let img = match image::open(&config.input_path) {
        Ok(img) => img,
        Err(e) => {
            warn!(
                "image_reader_system: falha ao abrir '{:?}': {e}",
                config.input_path
            );
            state.should_stop = true;
            return;
        }
    };

    let meta = FrameMeta {
        index: config.next_index,
        timestamp_us: 0,
        source: config.input_path.to_string_lossy().into_owned(),
    };
    let frame = Frame::from_dynamic_image(meta, img);
    info!(
    index = config.next_index,
    path = ?config.input_path,
    "image_reader_system: spawned frame {}x{}x{}",
    frame.height(), frame.width(), frame.channels()
    );

    commands.spawn(frame);
    config.next_index += 1;
}
