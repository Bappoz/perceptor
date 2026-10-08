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
//! O frame **deve** ser grayscale (1 canal). Use `grayscale_system` antes
//! ou adicione filtros com `.before()`/`.after()` no schedule.
//!

use bevy_ecs::prelude::*;
use perceptor_imgproc::sobel::apply_sobel;
use tracing::trace;

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
    /// Magnitude do gradiente `[H, W, 1]`, valores `u8` em `[0, 255]`.
    pub magnitude: ndarray::Array3<u8>,
}

/// Sistema ECS: computa o mapa de bordas Sobel para frames grayscale.
///
/// Requer [`GrayscaleTag`] — só processa frames já convertidos para cinza.
/// Registrado no `ProcessStage` pelo [`FiltersPlugin`].
pub fn sobel_system(
    mut query: Query<(Entity, &Frame), (With<GrayscaleTag>, Without<SobelTag>)>,
    mut commands: Commands,
) {
    for (entity, frame) in query.iter() {
        if frame.channels() != 1 {
            continue;
        }

        trace!(
            entity = ?entity,
            index = frame.meta.index,
            "sobel_system: computando bordas {}x{}",
            frame.height(),
            frame.width()
        );

        let magnitude = apply_sobel(&frame.data);

        commands
            .entity(entity)
            .insert((SobelMap { magnitude }, SobelTag));
    }
}
