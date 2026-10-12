//! `Pipeline` e `PipelineBuilder`: ponto de entrada do motor ECS.
//!
//! O `Pipeline` encapsula o `bevy_ecs::World` e um único `Schedule` em que
//! cada [`Stage`] é um `SystemSet`. A cada `tick()` o schedule roda uma vez:
//! os stages em ordem, com os `Commands` de um aplicados antes do seguinte.

use std::time::Instant;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{Schedule, SingleThreadedExecutor};
use bevy_ecs::system::ScheduleSystem;
use perceptor_core::{Error, Result};
use tracing::{debug, info, warn};

use crate::errors::{
    discard_failed_frames, has_frames_to_discard, not_halted, ErrorPolicy, PipelineErrors,
};
use crate::frame::Frame;
#[cfg(feature = "metrics")]
use crate::metrics::{add_boundary_markers, Boundary, PipelineMetrics};
use crate::ops::{IntoOpSystem, OpSet};
use crate::plugin::Plugin;
use crate::schedule::Stage;
use crate::stop::{count_new_frames, Sources, StopHandle, StopLimits, StopReason};

/// Motor principal do pipeline de CV.
///
/// Mantém o `World` do ECS e o `Schedule` com todos os stages.
/// Use [`PipelineBuilder`] para construí-lo.
pub struct Pipeline {
    world: World,
    schedule: Schedule,
    limits: StopLimits,
    stop_handle: StopHandle,
    /// Início do primeiro tick: referência do limite de duração.
    started: Option<Instant>,
    /// Contagem de Ctrl-C no `build`, se o pipeline os observa.
    #[cfg(feature = "ctrlc")]
    ctrl_c_baseline: Option<u64>,
    /// Plugins na ordem de registro; esvaziado por [`Pipeline::shutdown`].
    plugins: Vec<Box<dyn Plugin>>,
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Pipeline {
    /// Retorna um builder para configurar o pipeline com plugins e sistemas.
    #[must_use]
    pub fn builder() -> PipelineBuilder {
        PipelineBuilder::default()
    }

    /// Executa um único tick: roda todos os stages em ordem.
    ///
    /// Os erros reportados pelos sistemas ficam em [`Pipeline::errors`] até o
    /// tick seguinte. O tick conta em [`PipelineState::tick_count`] mesmo
    /// quando falha, e as [condições de parada](crate::stop) são avaliadas no
    /// fim dele. `tick()` não as consulta antes de rodar: quem as respeita é
    /// [`run`](Self::run).
    ///
    /// # Errors
    /// Em [`ErrorPolicy::FailFast`], o primeiro erro reportado por um sistema
    /// neste tick. Nas outras políticas nunca falha.
    pub fn tick(&mut self) -> Result<()> {
        let tick = self.world.resource::<PipelineState>().tick_count + 1;
        let _span = tracing::debug_span!("tick", tick).entered();
        let started = *self.started.get_or_insert_with(Instant::now);
        self.world.resource_mut::<PipelineErrors>().begin_tick();
        #[cfg(feature = "metrics")]
        let tick_started = self.world.resource_mut::<PipelineMetrics>().begin_tick();

        self.schedule.run(&mut self.world);

        #[cfg(feature = "metrics")]
        {
            let dropped = self.world.resource::<PipelineErrors>().discarded_frames();
            self.world
                .resource_mut::<PipelineMetrics>()
                .end_tick(tick_started, dropped);
        }

        self.world.resource_mut::<PipelineState>().tick_count = tick;
        if let Some(reason) = self.stop_condition(started) {
            self.world.resource_mut::<PipelineState>().stop_reason = Some(reason);
        }

        let mut errors = self.world.resource_mut::<PipelineErrors>();
        for failure in errors.iter() {
            warn!(system = failure.system, frame = ?failure.frame, "{}", failure.error);
        }
        if errors.policy() == ErrorPolicy::FailFast {
            if let Some(first) = errors.take_first() {
                return Err(first.error);
            }
        }
        Ok(())
    }

    /// Loop principal: executa ticks até uma [condição de parada](crate::stop)
    /// valer, e devolve qual foi.
    ///
    /// Em um pipeline já parado não roda nenhum tick. Sem fonte finita, limite
    /// ou pedido de parada, não retorna.
    ///
    /// # Errors
    /// Propaga o primeiro erro retornado por [`Pipeline::tick`], o que só
    /// acontece em [`ErrorPolicy::FailFast`].
    pub fn run(&mut self) -> Result<StopReason> {
        info!("Perceptor pipeline started");
        loop {
            if let Some(reason) = self.stop_reason() {
                info!(?reason, "Pipeline stopped");
                return Ok(reason);
            }
            self.tick()?;
        }
    }

    /// Por que o pipeline parou, ou `None` se ainda não parou.
    ///
    /// # Panics
    /// Se o recurso [`PipelineState`] tiver sido removido do `World`.
    #[must_use]
    pub fn stop_reason(&self) -> Option<StopReason> {
        self.world.resource::<PipelineState>().stop_reason
    }

    /// Handle para pedir parada de fora do pipeline, de qualquer thread.
    #[must_use]
    pub fn stop_handle(&self) -> StopHandle {
        self.stop_handle.clone()
    }

    /// A condição de parada que vale agora, se o pipeline ainda não parou.
    fn stop_condition(&self, started: Instant) -> Option<StopReason> {
        let state = self.world.resource::<PipelineState>();
        if state.stop_reason.is_some() {
            return None;
        }
        if self.stop_handle.is_stopped() {
            return Some(StopReason::Requested);
        }
        #[cfg(feature = "ctrlc")]
        if self
            .ctrl_c_baseline
            .is_some_and(|baseline| crate::stop::ctrl_c::received() > baseline)
        {
            return Some(StopReason::Interrupted);
        }
        if self.world.resource::<Sources>().all_finished() {
            return Some(StopReason::EndOfStream);
        }
        self.limits.reached(state, started.elapsed())
    }

    /// Encerra o pipeline: chama [`Plugin::cleanup`] de cada plugin, na ordem
    /// inversa de registro.
    ///
    /// Roda uma única vez: chamadas seguintes e o `Drop` não repetem o
    /// cleanup. O `World` continua acessível depois.
    pub fn shutdown(&mut self) {
        while let Some(plugin) = self.plugins.pop() {
            debug!(plugin = plugin.name(), "plugin cleanup");
            plugin.cleanup(&mut self.world);
        }
    }

    /// Métricas acumuladas: latência por stage e por tick, fps e descartes.
    ///
    /// # Panics
    /// Se o recurso [`PipelineMetrics`] tiver sido removido do `World`.
    #[cfg(feature = "metrics")]
    #[must_use]
    pub fn metrics(&self) -> &PipelineMetrics {
        self.world.resource::<PipelineMetrics>()
    }

    /// Erros reportados pelos sistemas no tick mais recente.
    ///
    /// # Panics
    /// Se o recurso [`PipelineErrors`] tiver sido removido do `World`.
    #[must_use]
    pub fn errors(&self) -> &PipelineErrors {
        self.world.resource::<PipelineErrors>()
    }

    /// Acesso direto ao `World` (para testes e introspecção).
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Acesso mutável ao `World` (para injetar entidades e recursos).
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

// ── PipelineBuilder ────────────────────────────────────────────────────────────

/// Builder fluente para configurar um [`Pipeline`].
pub struct PipelineBuilder {
    world: World,
    plugins: Vec<Box<dyn Plugin>>,
    /// Primeiro nome de plugin registrado em duplicata, reportado em `build`.
    duplicate: Option<String>,
    schedule: Schedule,
    limits: StopLimits,
    #[cfg(feature = "ctrlc")]
    stop_on_ctrl_c: bool,
    /// Ops registradas com `add_op` até agora.
    op_count: usize,
    /// Última op de cada stage, para encadear a seguinte.
    last_op: [Option<OpSet>; Stage::ALL.len()],
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        let mut schedule = Schedule::default();
        // Explícito: sem isto, a feature `multi_threaded` do `bevy_ecs` ligada
        // por outra crate do grafo trocaria o executor. Os spans de stage
        // dependem de tudo rodar na thread que chamou `tick()`; o paralelismo
        // do Perceptor fica dentro dos kernels (rayon).
        schedule.set_executor(SingleThreadedExecutor::new());
        // Encadear os sets ordena os stages e faz o bevy inserir um ponto de
        // sincronização (flush de `Commands`) entre dois stages consecutivos.
        // A tupla sai de `Stage::ALL` para a ordem ter uma única fonte.
        let [input, pre, process, post, output, cleanup] = Stage::ALL;
        schedule.configure_sets((input, pre, process, post, output, cleanup).chain());

        // Um erro em `FailFast` impede os stages seguintes de rodar; em
        // `SkipFrame` os frames com erro saem antes do stage seguinte.
        for (i, stage) in Stage::ALL.into_iter().enumerate() {
            schedule.configure_sets(stage.run_if(not_halted));
            let discard = discard_failed_frames.run_if(has_frames_to_discard);
            // A contagem vem antes do descarte: um frame que falhou já no
            // `Input` também foi produzido pela fonte.
            let discard = if stage == Stage::Input {
                (count_new_frames, discard).chain()
            } else {
                discard
            };
            let discard = discard.after(stage);
            // O descarte conta no tempo do stage que falhou.
            #[cfg(feature = "metrics")]
            let discard = discard.before(Boundary(i + 1));
            match Stage::ALL.get(i + 1) {
                Some(&next) => schedule.add_systems(discard.before(next)),
                None => schedule.add_systems(discard),
            };
        }

        let mut world = World::default();
        world.init_resource::<PipelineErrors>();
        world.init_resource::<PipelineState>();
        world.init_resource::<Sources>();
        #[cfg(feature = "metrics")]
        {
            add_boundary_markers(&mut schedule);
            world.init_resource::<PipelineMetrics>();
        }
        Self {
            world,
            plugins: Vec::new(),
            duplicate: None,
            schedule,
            limits: StopLimits::default(),
            #[cfg(feature = "ctrlc")]
            stop_on_ctrl_c: false,
            op_count: 0,
            last_op: [None; Stage::ALL.len()],
        }
    }
}

impl PipelineBuilder {
    /// Para depois de `n` ticks ([`StopReason::TickLimit`]).
    #[must_use]
    pub fn with_max_ticks(mut self, n: u64) -> Self {
        self.limits.ticks = Some(n);
        self
    }

    /// Para quando [`PipelineState::frame_count`] chegar a `n`
    /// ([`StopReason::FrameLimit`]).
    ///
    /// O limite é verificado no fim do tick: uma fonte que cria vários frames
    /// por tick pode passar de `n`, e nenhum frame criado fica sem processar.
    #[must_use]
    pub fn with_max_frames(mut self, n: u64) -> Self {
        self.limits.frames = Some(n);
        self
    }

    /// Para no fim do primeiro tick que terminar `duration` ou mais depois do
    /// início do primeiro ([`StopReason::TimeLimit`]).
    #[must_use]
    pub fn with_max_duration(mut self, duration: std::time::Duration) -> Self {
        self.limits.duration = Some(duration);
        self
    }

    /// Para no fim do tick em andamento quando o processo recebe Ctrl-C
    /// ([`StopReason::Interrupted`]).
    ///
    /// O tratador é instalado em [`build`](Self::build), uma vez por processo,
    /// e vale para todos os pipelines que pedirem isto. Depois de instalado, o
    /// Ctrl-C deixa de encerrar o processo: quem chama `run()` decide o que
    /// fazer quando ele retorna.
    #[cfg(feature = "ctrlc")]
    #[must_use]
    pub fn stop_on_ctrl_c(mut self) -> Self {
        self.stop_on_ctrl_c = true;
        self
    }

    /// Define a reação a erros de sistemas. O padrão é [`ErrorPolicy::FailFast`].
    #[must_use]
    pub fn with_error_policy(mut self, policy: ErrorPolicy) -> Self {
        self.world
            .resource_mut::<PipelineErrors>()
            .set_policy(policy);
        self
    }

    /// Registra um plugin no pipeline.
    ///
    /// O plugin receberá `&mut self` do builder e poderá adicionar sistemas.
    /// Um plugin com `name()` já registrado não é construído, e
    /// [`build`](Self::build) falhará com [`Error::DuplicatePlugin`].
    #[must_use]
    pub fn add_plugin<P: Plugin>(mut self, plugin: P) -> Self {
        if self.plugins.iter().any(|p| p.name() == plugin.name()) {
            self.duplicate
                .get_or_insert_with(|| plugin.name().to_owned());
            return self;
        }
        plugin.build(&mut self);
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Registra um ou mais sistemas em `stage`.
    ///
    /// Sistemas do mesmo stage não têm ordem entre si. Para ordená-los, passe
    /// uma tupla encadeada — `(a, b).chain()` — ou use `.before()`/`.after()`;
    /// os `Commands` de `a` já estarão aplicados quando `b` rodar.
    pub fn add_system<M>(
        &mut self,
        stage: Stage,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.schedule.add_systems(systems.in_set(stage));
        self
    }

    /// Registra em `stage` um kernel puro como sistema, sem escrever o
    /// sistema à mão. Ver [`ops`](crate::ops) para as assinaturas aceitas.
    ///
    /// Ops do mesmo stage rodam na ordem em que foram registradas.
    pub fn add_op<M>(&mut self, stage: Stage, op: impl IntoOpSystem<M>) -> &mut Self {
        let set = OpSet(self.op_count);
        self.op_count += 1;
        let mut system = op.into_op_system().in_set(stage).in_set(set);
        // `Stage` é um enum sem campos, declarado na ordem de `Stage::ALL`.
        if let Some(previous) = self.last_op[stage as usize].replace(set) {
            system = system.after(previous);
        }
        self.schedule.add_systems(system);
        self
    }

    /// Atalho para [`add_system`](Self::add_system) com [`Stage::Input`].
    pub fn add_input_system<M>(
        &mut self,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.add_system(Stage::Input, system)
    }

    /// Atalho para [`add_system`](Self::add_system) com [`Stage::PreProcess`].
    pub fn add_pre_process_system<M>(
        &mut self,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.add_system(Stage::PreProcess, system)
    }

    /// Atalho para [`add_system`](Self::add_system) com [`Stage::Process`].
    pub fn add_process_system<M>(
        &mut self,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.add_system(Stage::Process, system)
    }

    /// Atalho para [`add_system`](Self::add_system) com [`Stage::PostProcess`].
    pub fn add_post_process_system<M>(
        &mut self,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.add_system(Stage::PostProcess, system)
    }

    /// Atalho para [`add_system`](Self::add_system) com [`Stage::Output`].
    pub fn add_output_system<M>(
        &mut self,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.add_system(Stage::Output, system)
    }

    /// Atalho para [`add_system`](Self::add_system) com [`Stage::Cleanup`], que
    /// roda depois de todos os sistemas de saída. É onde entidades já
    /// consumidas são removidas; ver [`despawn_processed_frames_system`].
    pub fn add_cleanup_system<M>(
        &mut self,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.add_system(Stage::Cleanup, system)
    }

    /// Constrói o [`Pipeline`] a partir da configuração atual.
    ///
    /// Valida os plugins e só então chama [`Plugin::finish`] de cada um, na
    /// ordem de registro.
    ///
    /// # Errors
    /// - [`Error::DuplicatePlugin`] se dois plugins tiverem o mesmo nome.
    /// - [`Error::MissingPluginDependency`] se um plugin declarar em
    ///   [`Plugin::dependencies`] um nome que não foi registrado.
    /// - [`Error::SignalHandler`] se `stop_on_ctrl_c` foi pedido e o processo
    ///   já tem outro tratador de Ctrl-C.
    pub fn build(mut self) -> Result<Pipeline> {
        if let Some(name) = self.duplicate.take() {
            return Err(Error::DuplicatePlugin { name });
        }
        for plugin in &self.plugins {
            let missing = plugin
                .dependencies()
                .iter()
                .find(|dep| !self.plugins.iter().any(|p| p.name() == **dep));
            if let Some(dependency) = missing {
                return Err(Error::MissingPluginDependency {
                    plugin: plugin.name().to_owned(),
                    dependency: (*dependency).to_owned(),
                });
            }
        }

        #[cfg(feature = "ctrlc")]
        let ctrl_c_baseline = self
            .stop_on_ctrl_c
            .then(crate::stop::ctrl_c::watch)
            .transpose()?;

        // Permite que plugins façam ajustes finais após todos serem registrados
        let plugins = std::mem::take(&mut self.plugins);
        for plugin in &plugins {
            plugin.finish(&mut self);
        }

        Ok(Pipeline {
            world: self.world,
            schedule: self.schedule,
            limits: self.limits,
            stop_handle: StopHandle::default(),
            started: None,
            #[cfg(feature = "ctrlc")]
            ctrl_c_baseline,
            plugins,
        })
    }

    /// Acesso mutável ao `World` durante a construção (para plugins inserirem recursos).
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

// ── Ciclo de vida dos frames ───────────────────────────────────────────────────

/// Sistema ECS: remove toda entidade com [`Frame`], com os demais componentes dela.
///
/// Registre em [`Stage::Cleanup`] ([`PipelineBuilder::add_cleanup_system`]) para que
/// um pipeline contínuo não acumule um frame por tick. O
/// [`IoPlugin`](crate::plugins::io::IoPlugin) já o registra, a menos que
/// `keep_frames` esteja ligado.
pub fn despawn_processed_frames_system(frames: Query<Entity, With<Frame>>, mut commands: Commands) {
    for entity in &frames {
        // `try_`: o sistema pode estar registrado mais de uma vez no mesmo stage.
        commands.entity(entity).try_despawn();
    }
}

// ── Recursos globais do pipeline ───────────────────────────────────────────────

/// Recurso com o progresso do pipeline e o pedido de parada.
///
/// Um sistema encerra o loop de [`Pipeline::run`] com
/// [`request_stop`](Self::request_stop); as demais condições estão em
/// [`stop`](crate::stop).
#[derive(Resource, Debug, Default)]
pub struct PipelineState {
    /// Ticks concluídos. Durante um tick, ainda não conta o tick em andamento.
    pub tick_count: u64,
    /// Frames criados desde o início, contados logo depois de `Stage::Input`.
    /// Um frame criado em outro stage entra na conta no tick seguinte.
    pub frame_count: u64,
    pub(crate) stop_reason: Option<StopReason>,
}

impl PipelineState {
    /// Pede que o pipeline pare ao fim do tick em andamento, com
    /// [`StopReason::Requested`]. Não tem efeito se ele já parou.
    pub fn request_stop(&mut self) {
        self.stop_reason.get_or_insert(StopReason::Requested);
    }

    /// `true` se o pipeline já tem um motivo para parar.
    #[must_use]
    pub fn should_stop(&self) -> bool {
        self.stop_reason.is_some()
    }

    /// Por que o pipeline parou, ou `None` se ainda não parou.
    #[must_use]
    pub fn stop_reason(&self) -> Option<StopReason> {
        self.stop_reason
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::FrameMeta;

    #[test]
    fn tick_count_increments_each_tick() {
        let mut pipeline = Pipeline::builder().build().unwrap();
        assert_eq!(pipeline.world().resource::<PipelineState>().tick_count, 0);
        pipeline.tick().unwrap();
        assert_eq!(pipeline.world().resource::<PipelineState>().tick_count, 1);
        pipeline.tick().unwrap();
        assert_eq!(pipeline.world().resource::<PipelineState>().tick_count, 2);
    }

    #[test]
    fn max_ticks_stops_pipeline_after_n_ticks() {
        let mut pipeline = Pipeline::builder().with_max_ticks(3).build().unwrap();
        pipeline.run().unwrap();
        let state = pipeline.world().resource::<PipelineState>();
        assert_eq!(state.tick_count, 3);
        assert!(state.should_stop());
    }

    #[derive(Resource, Default)]
    struct Seen(Vec<&'static str>);

    fn test_frame() -> Frame {
        let meta = FrameMeta::new(0, "test");
        Frame::new(
            meta,
            perceptor_core::Image::<perceptor_core::L8>::new(2, 2).unwrap(),
        )
    }

    #[test]
    fn cleanup_stage_runs_after_output_stage() {
        let mut builder = Pipeline::builder();
        builder.world_mut().init_resource::<Seen>();
        builder.add_cleanup_system(|mut seen: ResMut<Seen>| seen.0.push("cleanup"));
        builder.add_output_system(|mut seen: ResMut<Seen>| seen.0.push("output"));
        builder.add_input_system(|mut seen: ResMut<Seen>| seen.0.push("input"));
        let mut pipeline = builder.build().unwrap();

        pipeline.tick().unwrap();
        assert_eq!(
            pipeline.world().resource::<Seen>().0,
            ["input", "output", "cleanup"]
        );
    }

    #[test]
    fn despawn_system_removes_frames_only_after_output_saw_them() {
        #[derive(Component)]
        struct Unrelated;

        let mut builder = Pipeline::builder();
        builder.world_mut().init_resource::<Seen>();
        builder.add_output_system(|frames: Query<&Frame>, mut seen: ResMut<Seen>| {
            if !frames.is_empty() {
                seen.0.push("output saw frame");
            }
        });
        // Registrado duas vezes: dois plugins podem pedir a mesma limpeza.
        builder.add_cleanup_system(despawn_processed_frames_system);
        builder.add_cleanup_system(despawn_processed_frames_system);
        let mut pipeline = builder.build().unwrap();
        pipeline.world_mut().spawn(test_frame());
        let other = pipeline.world_mut().spawn(Unrelated).id();

        pipeline.tick().unwrap();

        let world = pipeline.world_mut();
        assert_eq!(world.resource::<Seen>().0, ["output saw frame"]);
        assert_eq!(world.query::<&Frame>().iter(world).count(), 0);
        assert!(
            world.get_entity(other).is_ok(),
            "só entidades com Frame saem"
        );
    }

    #[test]
    fn frames_persist_without_a_cleanup_system() {
        let mut pipeline = Pipeline::builder().build().unwrap();
        pipeline.world_mut().spawn(test_frame());
        pipeline.tick().unwrap();
        pipeline.tick().unwrap();
        let world = pipeline.world_mut();
        assert_eq!(world.query::<&Frame>().iter(world).count(), 1);
    }

    #[test]
    fn stages_run_in_canonical_order_regardless_of_registration_order() {
        let mut builder = Pipeline::builder();
        builder.world_mut().init_resource::<Seen>();
        // Registrados de trás para frente.
        builder.add_system(Stage::Cleanup, |mut s: ResMut<Seen>| s.0.push("cleanup"));
        builder.add_system(Stage::Output, |mut s: ResMut<Seen>| s.0.push("output"));
        builder.add_system(Stage::PostProcess, |mut s: ResMut<Seen>| s.0.push("post"));
        builder.add_system(Stage::Process, |mut s: ResMut<Seen>| s.0.push("process"));
        builder.add_system(Stage::PreProcess, |mut s: ResMut<Seen>| s.0.push("pre"));
        builder.add_system(Stage::Input, |mut s: ResMut<Seen>| s.0.push("input"));
        let mut pipeline = builder.build().unwrap();

        pipeline.tick().unwrap();
        assert_eq!(
            pipeline.world().resource::<Seen>().0,
            ["input", "pre", "process", "post", "output", "cleanup"]
        );
        assert_eq!(Stage::ALL.len(), 6);
    }

    #[test]
    fn commands_from_one_stage_are_visible_in_the_next_within_the_same_tick() {
        #[derive(Component)]
        struct Marked;

        let mut builder = Pipeline::builder();
        builder.world_mut().init_resource::<Seen>();
        builder.add_system(Stage::Input, |mut commands: Commands| {
            commands.spawn(Marked);
        });
        builder.add_system(
            Stage::PreProcess,
            |marked: Query<(), With<Marked>>, mut seen: ResMut<Seen>| {
                if !marked.is_empty() {
                    seen.0.push("pre saw it");
                }
            },
        );
        let mut pipeline = builder.build().unwrap();

        pipeline.tick().unwrap();
        assert_eq!(pipeline.world().resource::<Seen>().0, ["pre saw it"]);
    }

    #[test]
    fn systems_in_the_same_stage_can_be_ordered() {
        #[derive(Component)]
        struct Marked;

        fn first(mut commands: Commands) {
            commands.spawn(Marked);
        }
        fn second(marked: Query<(), With<Marked>>, mut seen: ResMut<Seen>) {
            if !marked.is_empty() {
                seen.0.push("second saw it");
            }
        }

        let mut builder = Pipeline::builder();
        builder.world_mut().init_resource::<Seen>();
        builder.add_system(Stage::Process, (first, second).chain());
        let mut pipeline = builder.build().unwrap();

        pipeline.tick().unwrap();
        assert_eq!(pipeline.world().resource::<Seen>().0, ["second saw it"]);
    }
}
