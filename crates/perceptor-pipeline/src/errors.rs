//! Erros de sistemas: coleta em [`PipelineErrors`] e política de reação.
//!
//! Um sistema do `bevy_ecs` não tem para quem devolver um `Result`. Aqui ele
//! o entrega ao recurso [`PipelineErrors`], de duas formas:
//!
//! - retornando `perceptor_core::Result<()>` e sendo registrado com
//!   [`fallible`], quando a falha é do sistema inteiro (ex.: a fonte não abre);
//! - chamando [`PipelineErrors::report_frame`], quando a falha é de um frame
//!   e os demais devem continuar sendo processados.
//!
//! O que acontece depois é decidido pela [`ErrorPolicy`] do pipeline.

use bevy_ecs::prelude::*;
use perceptor_core::{Error, Result};

/// O que o pipeline faz quando um sistema reporta um erro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErrorPolicy {
    /// Os stages seguintes não rodam e [`Pipeline::tick`](crate::Pipeline::tick)
    /// retorna o primeiro erro. É o padrão.
    #[default]
    FailFast,
    /// O frame que causou o erro é removido ao fim do stage em que falhou; os
    /// demais seguem. Erros sem frame associado só ficam registrados.
    SkipFrame,
    /// O erro fica registrado e nada mais muda: o frame segue pelos stages.
    Continue,
}

/// Um erro reportado por um sistema durante um tick.
#[derive(Debug)]
pub struct SystemError {
    /// Nome do sistema que falhou.
    pub system: &'static str,
    /// Frame em que a falha ocorreu, quando ela é de um frame específico.
    pub frame: Option<Entity>,
    /// O erro em si.
    pub error: Error,
}

/// Recurso com os erros reportados no tick mais recente.
///
/// É esvaziado no início de cada tick; [`total`](Self::total) conta todos os
/// erros desde a criação do pipeline. Sistemas que o recebem como `ResMut`
/// não rodam em paralelo entre si.
#[derive(Resource, Debug, Default)]
pub struct PipelineErrors {
    policy: ErrorPolicy,
    current: Vec<SystemError>,
    total: u64,
}

impl PipelineErrors {
    /// Cria o recurso com a política dada.
    ///
    /// O [`PipelineBuilder`](crate::PipelineBuilder) já o insere; use isto ao
    /// rodar sistemas do Perceptor em um `World` próprio.
    #[must_use]
    pub fn new(policy: ErrorPolicy) -> Self {
        Self {
            policy,
            ..Self::default()
        }
    }

    /// Política em vigor.
    #[must_use]
    pub fn policy(&self) -> ErrorPolicy {
        self.policy
    }

    /// Registra um erro que não pertence a um frame específico.
    pub fn report(&mut self, system: &'static str, error: Error) {
        self.push(system, None, error);
    }

    /// Registra um erro ocorrido ao processar `frame`.
    ///
    /// Em [`ErrorPolicy::SkipFrame`] a entidade é removida ao fim do stage.
    pub fn report_frame(&mut self, system: &'static str, frame: Entity, error: Error) {
        self.push(system, Some(frame), error);
    }

    /// Erros do tick mais recente, na ordem em que foram reportados.
    ///
    /// Em [`ErrorPolicy::FailFast`] o primeiro não aparece aqui: ele é o
    /// valor retornado por [`Pipeline::tick`](crate::Pipeline::tick).
    pub fn iter(&self) -> impl Iterator<Item = &SystemError> {
        self.current.iter()
    }

    /// Quantidade de erros do tick mais recente.
    #[must_use]
    pub fn len(&self) -> usize {
        self.current.len()
    }

    /// `true` se o tick mais recente não teve erros.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.current.is_empty()
    }

    /// Erros reportados desde a criação do pipeline.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.total
    }

    fn push(&mut self, system: &'static str, frame: Option<Entity>, error: Error) {
        self.total += 1;
        self.current.push(SystemError {
            system,
            frame,
            error,
        });
    }

    pub(crate) fn set_policy(&mut self, policy: ErrorPolicy) {
        self.policy = policy;
    }

    pub(crate) fn begin_tick(&mut self) {
        self.current.clear();
    }

    /// Remove e devolve o primeiro erro do tick.
    pub(crate) fn take_first(&mut self) -> Option<SystemError> {
        (!self.current.is_empty()).then(|| self.current.remove(0))
    }
}

/// Adapta um sistema que retorna `Result<()>` para registro no pipeline.
///
/// Um `Err` vai para [`PipelineErrors`] com o nome do tipo do sistema e sem
/// frame associado. O resultado pode ser encadeado como qualquer sistema:
/// `(fallible(a), b).chain()`.
pub fn fallible<S, M>(system: S) -> impl System<In = (), Out = ()>
where
    S: IntoSystem<(), Result<()>, M> + 'static,
{
    IntoSystem::into_system(system.pipe(record_failure::<S>))
}

fn record_failure<S: 'static>(In(result): In<Result<()>>, mut errors: ResMut<PipelineErrors>) {
    if let Err(error) = result {
        errors.report(std::any::type_name::<S>(), error);
    }
}

/// Condição de execução dos stages: falsa depois de um erro em `FailFast`.
pub(crate) fn not_halted(errors: Res<PipelineErrors>) -> bool {
    errors.policy != ErrorPolicy::FailFast || errors.current.is_empty()
}

/// Condição de execução de [`discard_failed_frames`].
pub(crate) fn has_frames_to_discard(errors: Res<PipelineErrors>) -> bool {
    errors.policy == ErrorPolicy::SkipFrame && errors.current.iter().any(|e| e.frame.is_some())
}

/// Remove os frames com erro reportado. Roda entre dois stages consecutivos.
pub(crate) fn discard_failed_frames(errors: Res<PipelineErrors>, mut commands: Commands) {
    for frame in errors.current.iter().filter_map(|e| e.frame) {
        // `try_`: o frame pode já ter saído em um stage anterior deste tick.
        commands.entity(frame).try_despawn();
    }
}
