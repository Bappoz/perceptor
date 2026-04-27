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
use ndarray::Array3;
use rayon::prelude::*;
use tracing::trace;

use crate::core::frame::Frame;

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

// ── Lógica de conversão (implementar aqui) ─────────────────────────────────────

/// Converte tensor RGB `[H, W, 3]` para luminância `[H, W, 1]`.
///
/// # Panics
/// Panic se `input.shape()[2] != 3`.
pub fn convert_to_gray(input: &Array3<u8>) -> Array3<u8> {
    assert_eq!(input.shape()[2], 3, "esperado tensor RGB [H, W, 3]");
    let h = input.shape()[0];
    let w = input.shape()[1];

    let flat: Vec<u8> = input
        .as_slice()
        .expect("convert_to_gray: array não é contíguo")
        .par_chunks(3)
        .map(|px| (0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32) as u8)
        .collect();

    ndarray::Array3::from_shape_vec((h, w, 1), flat).expect("convert_to_gray: shape inválido")
}
