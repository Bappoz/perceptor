//! Filtro de detecção de bordas via operador de Sobel.
//!
//! # Algoritmo
//! Aplica convolução 3×3 separada nos eixos X e Y, combinando com:
//! ```text
//! G = sqrt(Gx² + Gy²)   (magnitude do gradiente)
//! ```
//!
//! Kernels canônicos:
//! ```text
//! Gx = [[-1,  0,  1],   Gy = [[-1, -2, -1],
//!        [-2,  0,  2],         [ 0,  0,  0],
//!        [-1,  0,  1]]          [ 1,  2,  1]]
//! ```
//!
//! # Pré-requisito
//! O frame **deve** estar em `L8` e ter [`GrayscaleTag`]. Como o tag
//! é inserido via `Commands`, ordene os dois: `(grayscale_system, sobel_system).chain()`.
//!

use bevy_ecs::prelude::*;
use perceptor_core::{Image, L8};
use perceptor_imgproc::sobel::apply_sobel;
use tracing::trace;

use crate::errors::PipelineErrors;
use crate::frame::Frame;
use crate::plugins::filters::grayscale::GrayscaleTag;

/// Componente marcador: indica que bordas Sobel foram computadas para este frame.
#[derive(Component, Debug, Default)]
pub struct SobelTag;

/// Componente que armazena o mapa de bordas Sobel separado do frame original.
///
/// Mantemos separado para não destruir o frame grayscale — outros sistemas
/// podem precisar dos dados originais.
#[derive(Component, Debug)]
pub struct SobelMap {
    /// Magnitude do gradiente, saturada em `[0, 255]`, nas dimensões do frame.
    pub magnitude: Image<L8>,
}

/// Sistema ECS: computa o mapa de bordas Sobel para frames grayscale.
///
/// Requer [`GrayscaleTag`] — só processa frames já convertidos para cinza.
/// Registrado em `Stage::Process` pelo [`FiltersPlugin`](crate::plugins::filters::FiltersPlugin).
/// Falhas do kernel são reportadas em [`PipelineErrors`] como erro do frame.
pub fn sobel_system(
    query: Query<(Entity, &Frame), (With<GrayscaleTag>, Without<SobelTag>)>,
    mut commands: Commands,
    mut errors: ResMut<PipelineErrors>,
) {
    for (entity, frame) in query.iter() {
        let Ok(gray) = frame.as_image::<L8>() else {
            continue;
        };

        trace!(
            entity = ?entity,
            index = frame.meta.index,
            "sobel_system: computando bordas {}x{}",
            frame.height(),
            frame.width()
        );

        match apply_sobel(gray.view()) {
            Ok(magnitude) => {
                commands
                    .entity(entity)
                    .insert((SobelMap { magnitude }, SobelTag));
            }
            Err(e) => errors.report_frame("sobel_system", entity, e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::Stage;
    use crate::testing::TestPipeline;
    use perceptor_core::Rgb8;

    /// Degrau vertical: colunas 0–1 pretas, 2–3 com valor 10.
    fn step() -> Image<L8> {
        Image::from_fn(4, 3, |x, _| L8::new(if x >= 2 { 10 } else { 0 })).unwrap()
    }

    #[test]
    fn computes_the_edge_map_of_a_tagged_gray_frame() {
        let mut test = TestPipeline::with_systems(Stage::Process, sobel_system);
        let frame = test.spawn(step());
        test.world_mut().entity_mut(frame).insert(GrayscaleTag);

        test.tick().unwrap();

        test.assert_has_all::<(SobelTag, SobelMap)>(frame);
        let magnitude = &test.component::<SobelMap>(frame).unwrap().magnitude;
        assert_eq!(magnitude.dimensions(), (4, 3));
        // Gx = (10 + 2·10 + 10) − 0 no pixel central da borda; Gy = 0.
        assert_eq!(magnitude.get(1, 1), Some(&L8::new(40)));
        // O frame original não é alterado.
        assert_eq!(test.image::<L8>(frame).as_pixels(), step().as_pixels());
    }

    #[test]
    fn requires_the_grayscale_tag() {
        let mut test = TestPipeline::with_systems(Stage::Process, sobel_system);
        let untagged = test.spawn(step());

        test.tick().unwrap();

        test.assert_lacks_all::<(SobelTag, SobelMap)>(untagged);
    }

    #[test]
    fn ignores_tagged_frames_in_another_format() {
        let mut test = TestPipeline::with_systems(Stage::Process, sobel_system);
        let frame = test.spawn(Image::<Rgb8>::new(2, 2).unwrap());
        test.world_mut().entity_mut(frame).insert(GrayscaleTag);

        test.tick().unwrap();

        test.assert_lacks_all::<(SobelTag, SobelMap)>(frame);
        assert!(test.errors().is_empty());
    }
}
