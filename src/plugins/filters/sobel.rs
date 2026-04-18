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
//! # TODO: implementar a convolução real
//! Use `ndarray` com janelas deslizantes ou `rayon` para paralelismo.

use bevy_ecs::prelude::*;
use tracing::trace;

use crate::core::frame::Frame;
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

        // TODO: implementar convolução Sobel real
        // let magnitude = apply_sobel(&frame.data);
        let magnitude = ndarray::Array3::zeros((frame.height(), frame.width(), 1));

        commands.entity(entity).insert((
            SobelMap { magnitude },
            SobelTag,
        ));
    }
}

// ── Kernels e convolução (implementar aqui) ────────────────────────────────────

/// Kernels do operador Sobel 3×3.
#[allow(dead_code)]
const KERNEL_GX: [[i8; 3]; 3] = [
    [-1, 0, 1],
    [-2, 0, 2],
    [-1, 0, 1],
];

#[allow(dead_code)]
const KERNEL_GY: [[i8; 3]; 3] = [
    [-1, -2, -1],
    [ 0,  0,  0],
    [ 1,  2,  1],
];

/// Aplica os kernels Sobel e retorna a magnitude do gradiente `[H, W, 1]`.
///
/// # Panics
/// Panic se `input.shape()[2] != 1` (deve ser grayscale).
#[allow(dead_code)]
fn apply_sobel(input: &ndarray::Array3<u8>) -> ndarray::Array3<u8> {
    assert_eq!(input.shape()[2], 1, "Sobel requer frame grayscale [H, W, 1]");
    let (h, w) = (input.shape()[0], input.shape()[1]);

    // TODO: implementar convolução com borda zero-padding
    ndarray::Array3::zeros((h, w, 1))
}
