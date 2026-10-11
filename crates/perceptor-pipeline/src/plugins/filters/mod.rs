//! Plugin de filtros clássicos de CV.
//!
//! # Sistemas disponíveis
//! | Sistema            | Stage          | Descrição                        |
//! |--------------------|----------------|----------------------------------|
//! | `grayscale_system` | `Stage::Process` | RGB → Luminância (BT.601)        |
//! | `sobel_system`     | `Stage::Process` | Detecção de bordas (Sobel 3×3)   |
//!
//! Com os dois habilitados o Sobel é ordenado depois do grayscale e roda no mesmo tick.
//!
//! # Adicionando filtros customizados
//! Implemente uma função com assinatura de sistema ECS e registre-a:
//! ```rust,ignore
//! builder.add_system(Stage::Process, my_filter_system);
//! ```

pub mod grayscale;
pub mod sobel;

use bevy_ecs::schedule::IntoScheduleConfigs;

use crate::{pipeline::PipelineBuilder, plugin::Plugin, schedule::Stage};
use grayscale::grayscale_system;
use sobel::sobel_system;

/// Plugin que registra os filtros clássicos em [`Stage::Process`].
///
/// `Default` não habilita nenhum filtro (equivale a [`FiltersPlugin::none`]);
/// use [`FiltersPlugin::all`] para habilitar grayscale e Sobel.
#[derive(Debug, Default)]
pub struct FiltersPlugin {
    /// Registra `grayscale_system` em [`Stage::Process`].
    pub enable_grayscale: bool,
    /// Registra `sobel_system` em [`Stage::Process`].
    pub enable_sobel: bool,
}

impl FiltersPlugin {
    /// Habilita todos os filtros disponíveis.
    #[must_use]
    pub fn all() -> Self {
        Self {
            enable_grayscale: true,
            enable_sobel: true,
        }
    }

    /// Sem filtros pré-habilitados (configure manualmente).
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }
}

impl Plugin for FiltersPlugin {
    fn name(&self) -> &'static str {
        "FiltersPlugin"
    }

    fn build(&self, builder: &mut PipelineBuilder) {
        match (self.enable_grayscale, self.enable_sobel) {
            // Encadeados: o Sobel exige o `GrayscaleTag` que o grayscale insere via `Commands`.
            (true, true) => {
                builder.add_system(Stage::Process, (grayscale_system, sobel_system).chain())
            }
            (true, false) => builder.add_system(Stage::Process, grayscale_system),
            (false, true) => builder.add_system(Stage::Process, sobel_system),
            (false, false) => builder,
        };
    }
}
