//! Plugin de filtros clássicos de CV.
//!
//! # Sistemas disponíveis
//! | Sistema            | Stage          | Descrição                        |
//! |--------------------|----------------|----------------------------------|
//! | `grayscale_system` | ProcessStage   | RGB → Luminância (BT.601)        |
//! | `sobel_system`     | ProcessStage   | Detecção de bordas (Sobel 3×3)   |
//!
//! # Adicionando filtros customizados
//! Implemente uma função com assinatura de sistema ECS e registre-a:
//! ```rust,ignore
//! builder.add_process_system(my_filter_system);
//! ```

pub mod grayscale;
pub mod sobel;

use crate::core::{pipeline::PipelineBuilder, plugin::Plugin};
use grayscale::grayscale_system;
use sobel::sobel_system;

/// Plugin que registra os filtros clássicos no `ProcessStage`.
///
/// Por padrão habilita grayscale e Sobel. Use [`FiltersPlugin::none`]
/// para registrar apenas os filtros desejados manualmente.
#[derive(Debug, Default)]
pub struct FiltersPlugin {
    pub enable_grayscale: bool,
    pub enable_sobel: bool,
}

impl FiltersPlugin {
    /// Habilita todos os filtros disponíveis.
    pub fn all() -> Self {
        Self { enable_grayscale: true, enable_sobel: true }
    }

    /// Sem filtros pré-habilitados (configure manualmente).
    pub fn none() -> Self {
        Self::default()
    }
}

impl Plugin for FiltersPlugin {
    fn name(&self) -> &str { "FiltersPlugin" }

    fn build(&self, builder: &mut PipelineBuilder) {
        if self.enable_grayscale {
            builder.add_process_system(grayscale_system);
        }
        if self.enable_sobel {
            builder.add_process_system(sobel_system);
        }
    }
}
