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
use tracing::warn;

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
///
/// Registrado no `InputStage` pelo [`IoPlugin`].
///
/// # TODO
/// Implementar leitura real. Atualmente é um no-op com aviso de log.
pub fn image_reader_system(
    mut commands: Commands,
    mut config: ResMut<IoConfig>,
) {
    if config.source.is_empty() {
        warn!("image_reader_system: IoConfig.source não configurado, pulando tick");
        return;
    }

    // TODO: ler frame real de `config.source`
    // Exemplo de implementação futura:
    //
    // let img = image::open(&config.source)?;
    // let meta = FrameMeta {
    //     index: config.next_index,
    //     timestamp_us: current_timestamp_us(),
    //     source: config.source.clone(),
    // };
    // let frame = Frame::from_dynamic_image(meta, img);
    // commands.spawn(frame);
    // config.next_index += 1;

    let _ = &mut commands; // silencia warning unused enquanto TODO
    let _ = &mut config;
}
