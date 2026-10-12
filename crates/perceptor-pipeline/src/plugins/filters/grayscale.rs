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
    use crate::schedule::Stage;
    use crate::testing::TestPipeline;
    use perceptor_core::{Image, PixelFormat, L8};

    #[test]
    fn converts_rgb_frames_and_tags_them() {
        let mut test = TestPipeline::with_systems(Stage::Process, grayscale_system);
        let frame = test.spawn(Image::filled(2, 2, Rgb8::new(255, 0, 0)).unwrap());

        test.tick().unwrap();

        test.assert_has::<GrayscaleTag>(frame);
        assert_eq!(test.frame(frame).format(), PixelFormat::L8);
        // R puro: 0.299 · 255 = 76.245.
        assert_eq!(test.image::<L8>(frame).as_pixels(), [L8::new(76); 4]);
    }

    #[test]
    fn ignores_frames_that_are_not_rgb() {
        let mut test = TestPipeline::with_systems(Stage::Process, grayscale_system);
        let frame = test.spawn(Image::filled(2, 2, L8::new(9)).unwrap());

        test.tick().unwrap();

        test.assert_lacks::<GrayscaleTag>(frame);
        assert_eq!(test.image::<L8>(frame).as_pixels(), [L8::new(9); 4]);
        assert!(test.errors().is_empty());
    }

    #[test]
    fn does_not_reprocess_a_tagged_frame() {
        let mut test = TestPipeline::with_systems(Stage::Process, grayscale_system);
        let frame = test.spawn(Image::<Rgb8>::new(2, 2).unwrap());
        test.tick().unwrap();

        // Um sistema posterior devolve o frame a RGB; o tag impede nova conversão.
        test.world_mut().get_mut::<Frame>(frame).unwrap().data =
            Image::<Rgb8>::new(2, 2).unwrap().into();
        test.tick().unwrap();

        assert_eq!(test.frame(frame).format(), PixelFormat::Rgb8);
    }
}
