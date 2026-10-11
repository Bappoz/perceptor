//! Filtro de conversão para escala de cinza.
//!
//! # Algoritmo
//! Usa os coeficientes de luminância BT.601 (padrão SDTV):
//! ```text
//! Y = 0.299·R + 0.587·G + 0.114·B
//! ```
//!
//! # Comportamento ECS
//! Itera sobre todas as entidades com um `Frame` em `Rgb8` e substitui
//! `frame.data` por uma imagem `L8`. Frames em outros formatos são ignorados.
//!

use bevy_ecs::prelude::*;
use perceptor_core::Rgb8;
use perceptor_imgproc::grayscale::convert_to_gray;
use tracing::trace;

use crate::errors::PipelineErrors;
use crate::frame::Frame;

/// Componente marcador: indica que o frame foi convertido para grayscale.
/// Permite que outros sistemas filtrem apenas frames já processados.
#[derive(Component, Debug, Default)]
pub struct GrayscaleTag;

/// Sistema ECS: converte frames RGB em luminância (grayscale).
///
/// Registrado em `Stage::Process` pelo [`FiltersPlugin`](crate::plugins::filters::FiltersPlugin).
/// Falhas do kernel são reportadas em [`PipelineErrors`] como erro do frame.
pub fn grayscale_system(
    mut query: Query<(Entity, &mut Frame), Without<GrayscaleTag>>,
    mut commands: Commands,
    mut errors: ResMut<PipelineErrors>,
) {
    for (entity, mut frame) in &mut query {
        let Ok(rgb) = frame.as_image::<Rgb8>() else {
            continue; // Ignora frames que não são RGB
        };

        trace!(
            entity = ?entity,
            index = frame.meta.index,
            "grayscale_system: convertendo frame {}x{}",
            frame.height(),
            frame.width()
        );

        match convert_to_gray(rgb.view()) {
            Ok(gray) => {
                frame.data = gray.into();
                // Marca o frame como processado para evitar re-processamento
                commands.entity(entity).insert(GrayscaleTag);
            }
            Err(e) => errors.report_frame("grayscale_system", entity, e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::FrameMeta;
    use perceptor_core::{Image, PixelFormat};

    #[test]
    fn grayscale_system_tags_entity() {
        let mut world = World::new();
        world.init_resource::<PipelineErrors>();

        let data = Image::filled(2, 2, Rgb8::new(200, 200, 200)).unwrap();
        world.spawn(Frame::new(FrameMeta::new(0, "test"), data));

        // Roda o sistema diretamente no world
        let mut schedule = Schedule::default();
        schedule.add_systems(grayscale_system);
        schedule.run(&mut world);

        // Verifica tag + shape do frame convertido
        let mut q = world.query::<(&Frame, &GrayscaleTag)>();
        let (frame, _) = q.single(&world).unwrap();
        assert_eq!(frame.format(), PixelFormat::L8);
    }
}
