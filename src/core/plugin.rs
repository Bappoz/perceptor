//! Trait `Plugin`: contrato para extensão do pipeline via plugins.
//!
//! Inspirado no sistema de plugins do Bevy, cada plugin recebe acesso
//! ao `PipelineBuilder` para registrar sistemas nos schedules corretos.
//!
//! # Exemplo de plugin customizado
//!
//! ```rust,no_run
//! use perceptor::core::{pipeline::PipelineBuilder, plugin::Plugin};
//!
//! pub struct MyPlugin;
//!
//! impl Plugin for MyPlugin {
//!     fn name(&self) -> &str { "MyPlugin" }
//!
//!     fn build(&self, builder: &mut PipelineBuilder) {
//!         // builder.add_system(ProcessStage, my_system);
//!     }
//! }
//! ```

use crate::core::pipeline::PipelineBuilder;

/// Trait que todo plugin deve implementar.
pub trait Plugin: Send + Sync + 'static {
    /// Nome único do plugin, usado para logging e detecção de duplicatas.
    fn name(&self) -> &str;

    /// Registra sistemas e recursos no `PipelineBuilder`.
    fn build(&self, builder: &mut PipelineBuilder);

    /// Chamado após todos os plugins serem registrados (útil para dependências).
    /// Implementação padrão é no-op.
    fn finish(&self, _builder: &mut PipelineBuilder) {}

    /// Chamado quando o pipeline é destruído.
    /// Implementação padrão é no-op.
    fn cleanup(&self, _builder: &mut PipelineBuilder) {}
}
