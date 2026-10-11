//! Trait `Plugin`: contrato para extensão do pipeline via plugins.
//!
//! Inspirado no sistema de plugins do Bevy, cada plugin recebe acesso
//! ao `PipelineBuilder` para registrar sistemas no [`Stage`](crate::Stage) correto.
//!
//! # Exemplo de plugin customizado
//!
//! ```rust,no_run
//! use perceptor_pipeline::{pipeline::PipelineBuilder, plugin::Plugin, Stage};
//!
//! pub struct MyPlugin;
//!
//! impl Plugin for MyPlugin {
//!     fn name(&self) -> &str { "MyPlugin" }
//!
//!     fn build(&self, builder: &mut PipelineBuilder) {
//!         builder.add_system(Stage::Process, || {});
//!     }
//! }
//! ```

use bevy_ecs::world::World;

use crate::pipeline::PipelineBuilder;

/// Trait que todo plugin deve implementar.
///
/// # Ciclo de vida
/// 1. [`build`](Self::build), em `add_plugin`, na ordem de registro;
/// 2. [`finish`](Self::finish), em `PipelineBuilder::build`, na ordem de
///    registro, depois de validadas duplicatas e dependências;
/// 3. [`cleanup`](Self::cleanup), em `Pipeline::shutdown` ou no `Drop` do
///    pipeline, na ordem **inversa** de registro, uma única vez.
pub trait Plugin: Send + Sync + 'static {
    /// Nome único do plugin. Registrar dois plugins com o mesmo nome faz
    /// `PipelineBuilder::build` falhar com
    /// [`Error::DuplicatePlugin`](perceptor_core::Error::DuplicatePlugin).
    fn name(&self) -> &str;

    /// Nomes dos plugins que precisam estar registrados no mesmo pipeline.
    ///
    /// A ordem de registro não importa: a validação acontece em
    /// `PipelineBuilder::build`, que falha com
    /// [`Error::MissingPluginDependency`](perceptor_core::Error::MissingPluginDependency)
    /// se algum faltar. Implementação padrão: nenhuma dependência.
    fn dependencies(&self) -> &[&str] {
        &[]
    }

    /// Registra sistemas e recursos no `PipelineBuilder`.
    fn build(&self, builder: &mut PipelineBuilder);

    /// Chamado após todos os plugins serem registrados e validados; os
    /// plugins declarados em [`dependencies`](Self::dependencies) já passaram
    /// por `build`. Implementação padrão é no-op.
    fn finish(&self, _builder: &mut PipelineBuilder) {}

    /// Chamado no encerramento do pipeline, para liberar o que o plugin
    /// mantém no `World` ou fora dele. Implementação padrão é no-op.
    fn cleanup(&self, _world: &mut World) {}
}
