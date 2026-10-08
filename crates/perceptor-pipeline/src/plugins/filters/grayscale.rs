//! Filtro de conversão para escala de cinza.
//!
//! # Algoritmo
//! Usa os coeficientes de luminância BT.601 (padrão SDTV):
//! ```text
//! Y = 0.299·R + 0.587·G + 0.114·B
//! ```
//!
//! # Comportamento ECS
//! Itera sobre todas as entidades com um `Frame` RGB (3 canais) e substitui
//! `frame.data` por um tensor `[H, W, 1]` em grayscale.
//!

use bevy_ecs::prelude::*;
use perceptor_imgproc::grayscale::convert_to_gray;
use tracing::trace;

use crate::frame::Frame;

/// Componente marcador: indica que o frame foi convertido para grayscale.
/// Permite que outros sistemas filtrem apenas frames já processados.
#[derive(Component, Debug, Default)]
pub struct GrayscaleTag;

/// Sistema ECS: converte frames RGB em luminância (grayscale).
///
/// Registrado no `ProcessStage` pelo [`FiltersPlugin`].
pub fn grayscale_system(
    mut query: Query<(Entity, &mut Frame), Without<GrayscaleTag>>,
    mut commands: Commands,
) {
    for (entity, mut frame) in query.iter_mut() {
        if frame.channels() != 3 {
            continue; // Ignora frames que não são RGB
        }

        trace!(
            entity = ?entity,
            index = frame.meta.index,
            "grayscale_system: convertendo frame {}x{}",
            frame.height(),
            frame.width()
        );

        frame.data = convert_to_gray(&frame.data);
        // Marca o frame como processado para evitar re-processamento
        commands.entity(entity).insert(GrayscaleTag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::FrameMeta;
    use ndarray::Array3;

    #[test]
    fn grayscale_system_tags_entity() {
        let mut world = World::new();

        let meta = FrameMeta {
            index: 0,
            timestamp_us: 0,
            source: "test".into(),
        };
        let data = Array3::from_elem((2, 2, 3), 200u8);
        world.spawn(Frame::new(meta, data));

        // Roda o sistema diretamente no world
        let mut schedule = Schedule::default();
        schedule.add_systems(grayscale_system);
        schedule.run(&mut world);

        // Verifica tag + shape do frame convertido
        let mut q = world.query::<(&Frame, &GrayscaleTag)>();
        let (frame, _) = q.single(&world);
        assert_eq!(frame.channels(), 1);
    }
}
