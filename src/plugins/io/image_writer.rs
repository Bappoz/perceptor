use crate::{
    core::{frame::Frame, pipeline::PipelineState},
    plugins::io::{config::ImageFormat, IoConfig},
};
use bevy_ecs::system::{Query, Res, ResMut};
use tracing::{info, warn};

/// Sistema ECS: salva todos os frames presentes no world no caminho configurado.
/// Registrado no `OutputStage` pelo [`IoPlugin`].
pub fn image_writer_system(
    query: Query<&Frame>,
    config: Res<IoConfig>,
    mut state: ResMut<PipelineState>,
) {
    if config.output_path.as_os_str().is_empty() {
        warn!("image_writer_system: output_path não configurado");
        state.should_stop = true;
        return;
    }

    let img_format = match config.format {
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
    };

    for frame in query.iter() {
        let (h, w, c) = (frame.height(), frame.width(), frame.channels());
        let raw = frame.data.as_slice().expect("Array não contígua");

        let color = if c == 1 {
            image::ColorType::L8
        } else {
            image::ColorType::Rgb8
        };

        match image::save_buffer_with_format(&config.output_path, raw, w as u32, h as u32, color, img_format) {
            Ok(_) => info!("image_writer_system: salvo em {:?}", config.output_path),
            Err(e) => warn!("image_writer_system: falha ao salvar: {e}"),
        }
    }
    state.should_stop = true;
}
