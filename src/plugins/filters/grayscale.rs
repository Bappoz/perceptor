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
//! Frames que já possuem 1 canal são ignorados.
//!
//! # TODO: implementar a transformação real
//! ```rust,no_run
//! use ndarray::Zip;
//! // Zip::from(out.lanes_mut(Axis(2)))
//! //     .and(frame.data.lanes(Axis(2)))
//! //     .par_for_each(|mut o, rgb| { ... });
//! ```

use bevy_ecs::prelude::*;
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

        // TODO: implementar conversão BT.601
        // let gray = convert_to_gray(&frame.data);
        // frame.data = gray;

        // Marca o frame como processado para evitar re-processamento
        commands.entity(entity).insert(GrayscaleTag);
    }
}

// ── Lógica de conversão (implementar aqui) ─────────────────────────────────────

/// Converte tensor RGB `[H, W, 3]` para luminância `[H, W, 1]`.
///
/// # Panics
/// Panic se `input.shape()[2] != 3`.
#[allow(dead_code)]
fn convert_to_gray(input: &ndarray::Array3<u8>) -> ndarray::Array3<u8> {
    assert_eq!(input.shape()[2], 3, "esperado tensor RGB [H, W, 3]");
    let (h, w, _) = (input.shape()[0], input.shape()[1], input.shape()[2]);

    // TODO: implementar com rayon para paralelismo por linha
    ndarray::Array3::zeros((h, w, 1))
}
