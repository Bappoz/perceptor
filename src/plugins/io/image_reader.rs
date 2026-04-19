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

/// Recurso que configura a fonte de I/O para o `IoPlugin`.
#[derive(Resource, Debug, Default)]
pub struct IoConfig {
    /// Caminho do arquivo de imagem/vídeo ou identificador de device.
    pub source: String,
    /// Índice do próximo frame a ser lido (gerenciado internamente).
    pub(crate) next_index: u64,
}

/// Sistema ECS: lê um frame da fonte configurada e o spawna como entidade.
/// Registrado no `InputStage` pelo [`IoPlugin`].
pub fn image_reader_system(
    mut commands: Commands,
    mut config: ResMut<IoConfig>,
    mut state: ResMut<crate::core::pipeline::PipelineState>,
) {
    if config.source.is_empty() {
        warn!("image_reader_system: IoConfig.source não configurado, pulando tick");
        state.should_stop = true;
        return;
    }

    let img = match image::open(&config.source) {
        Ok(img) => img,
        Err(e) => {
            warn!(
                "image_reader_system: falha ao abrir '{}': {e}",
                config.source
            );
            state.should_stop = true;
            return;
        }
    };

    let meta = FrameMeta {
        index: config.next_index,
        timestamp_us: 0,
        source: config.source.clone(),
    };
    let frame = Frame::from_dynamic_image(meta, img);
    info!(
    index = config.next_index,
    path = %config.source,
    "image_reader_system: spawned frame {}x{}x{}",
    frame.height(), frame.width(), frame.channels()
    );

    commands.spawn(frame);
    config.next_index += 1;

    state.should_stop = true;
}
