//! `Pipeline` e `PipelineBuilder`: ponto de entrada do motor ECS.
//!
//! O `Pipeline` encapsula o `bevy_ecs::World` e um único `Schedule` em que
//! cada [`Stage`] é um `SystemSet`. A cada `tick()` o schedule roda uma vez:
//! os stages em ordem, com os `Commands` de um aplicados antes do seguinte.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use bevy_ecs::system::ScheduleSystem;
use perceptor_core::Result;
use tracing::{debug, info};

use crate::frame::Frame;
use crate::plugin::Plugin;
use crate::schedule::Stage;

/// Motor principal do pipeline de CV.
///
/// Mantém o `World` do ECS e o `Schedule` com todos os stages.
/// Use [`PipelineBuilder`] para construí-lo.
pub struct Pipeline {
    world: World,
    schedule: Schedule,
    max_ticks: Option<u64>,
}

impl Pipeline {
    /// Retorna um builder para configurar o pipeline com plugins e sistemas.
    #[must_use]
    pub fn builder() -> PipelineBuilder {
        PipelineBuilder::default()
    }

    /// Executa um único tick: roda todos os stages em ordem.
    ///
    /// # Errors
    /// Hoje nunca falha: sistemas registram erros em log. A propagação real
    /// de erros é o objetivo da issue #40.
    pub fn tick(&mut self) -> Result<()> {
        debug!("pipeline tick");
        self.schedule.run(&mut self.world);

        let mut state = self.world.resource_mut::<PipelineState>();
        state.tick_count += 1;
        if let Some(max) = self.max_ticks {
            if state.tick_count >= max {
                state.should_stop = true;
            }
        }

        Ok(())
    }

    /// Loop principal: executa ticks até que um sistema de saída sinalize parada.
    ///
    /// A condição de parada é controlada pelo recurso [`PipelineState`].
    ///
    /// # Errors
    /// Propaga o primeiro erro retornado por [`Pipeline::tick`].
    pub fn run(&mut self) -> Result<()> {
        info!("Perceptor pipeline started");
        loop {
            self.tick()?;

            let state = self.world.resource::<PipelineState>();
            if state.should_stop {
                info!("Pipeline stopped by request");
                break;
            }
        }
        Ok(())
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
    schedule: Schedule,
    max_ticks: Option<u64>,
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        let mut schedule = Schedule::default();
        // Encadear os sets ordena os stages e faz o bevy inserir um ponto de
        // sincronização (flush de `Commands`) entre dois stages consecutivos.
        // A tupla sai de `Stage::ALL` para a ordem ter uma única fonte.
        let [input, pre, process, post, output, cleanup] = Stage::ALL;
        schedule.configure_sets((input, pre, process, post, output, cleanup).chain());
        Self {
            world: World::default(),
            plugins: Vec::new(),
            schedule,
            max_ticks: None,
        }
    }
}

impl PipelineBuilder {
    /// Define o número máximo de ticks antes de encerrar o pipeline automaticamente.
    ///
    /// Quando `tick_count >= n`, [`PipelineState::should_stop`] é definido como `true`.
    /// Útil para processamento em lote de N frames.
    #[must_use]
    pub fn with_max_ticks(mut self, n: u64) -> Self {
        self.max_ticks = Some(n);
        self
    }

    /// Registra um plugin no pipeline.
    ///
    /// O plugin receberá `&mut self` do builder e poderá adicionar sistemas.
    /// Plugins duplicados (mesmo `name()`) são ignorados com aviso.
    #[must_use]
    pub fn add_plugin<P: Plugin>(mut self, plugin: P) -> Self {
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
    pub fn build(mut self) -> Pipeline {
        // Inicializa recurso de estado do pipeline
        self.world.insert_resource(PipelineState::default());

        // Permite que plugins façam ajustes finais após todos serem registrados
        let plugins = std::mem::take(&mut self.plugins);
        for plugin in &plugins {
            plugin.finish(&mut self);
        }

        Pipeline {
            world: self.world,
            schedule: self.schedule,
            max_ticks: self.max_ticks,
        }
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

/// Recurso que controla o ciclo de vida do pipeline.
///
/// Sistemas podem definir `should_stop = true` para encerrar o loop principal.
#[derive(Resource, Debug, Default)]
pub struct PipelineState {
    /// Quando `true`, o loop em [`Pipeline::run`] termina após o tick atual.
    pub should_stop: bool,
    /// Contador de ticks executados (útil para benchmarks e logs).
    pub tick_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::FrameMeta;

    #[test]
    fn tick_count_increments_each_tick() {
        let mut pipeline = Pipeline::builder().build();
        assert_eq!(pipeline.world().resource::<PipelineState>().tick_count, 0);
        pipeline.tick().unwrap();
        assert_eq!(pipeline.world().resource::<PipelineState>().tick_count, 1);
        pipeline.tick().unwrap();
        assert_eq!(pipeline.world().resource::<PipelineState>().tick_count, 2);
    }

    #[test]
    fn max_ticks_stops_pipeline_after_n_ticks() {
        let mut pipeline = Pipeline::builder().with_max_ticks(3).build();
        pipeline.run().unwrap();
        let state = pipeline.world().resource::<PipelineState>();
        assert_eq!(state.tick_count, 3);
        assert!(state.should_stop);
    }

    #[derive(Resource, Default)]
    struct Seen(Vec<&'static str>);

    fn test_frame() -> Frame {
        let meta = FrameMeta {
            index: 0,
            timestamp_us: 0,
            source: "test".into(),
        };
        Frame::new(meta, ndarray::Array3::zeros((2, 2, 1)))
    }

    #[test]
    fn cleanup_stage_runs_after_output_stage() {
        let mut builder = Pipeline::builder();
        builder.world_mut().init_resource::<Seen>();
        builder.add_cleanup_system(|mut seen: ResMut<Seen>| seen.0.push("cleanup"));
        builder.add_output_system(|mut seen: ResMut<Seen>| seen.0.push("output"));
        builder.add_input_system(|mut seen: ResMut<Seen>| seen.0.push("input"));
        let mut pipeline = builder.build();

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
        let mut pipeline = builder.build();
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
        let mut pipeline = Pipeline::builder().build();
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
        let mut pipeline = builder.build();

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
        let mut pipeline = builder.build();

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
        let mut pipeline = builder.build();

        pipeline.tick().unwrap();
        assert_eq!(pipeline.world().resource::<Seen>().0, ["second saw it"]);
    }
}
