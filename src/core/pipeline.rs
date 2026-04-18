//! `Pipeline` e `PipelineBuilder`: ponto de entrada do motor ECS.
//!
//! O `Pipeline` encapsula o `bevy_ecs::World` e os `Schedule`s de cada
//! fase. A cada chamada de `tick()` ele executa os stages em ordem,
//! processando todos os frames que estiverem no world naquele momento.

use anyhow::Result;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use tracing::{debug, info};

use crate::core::{
    plugin::Plugin,
    schedule::{InputStage, OutputStage, PostProcessStage, PreProcessStage, ProcessStage},
};

/// Motor principal do pipeline de CV.
///
/// Mantém o `World` do ECS e os `Schedule`s de cada fase.
/// Use [`PipelineBuilder`] para construí-lo.
pub struct Pipeline {
    world: World,
    input_schedule: Schedule,
    pre_process_schedule: Schedule,
    process_schedule: Schedule,
    post_process_schedule: Schedule,
    output_schedule: Schedule,
}

impl Pipeline {
    /// Retorna um builder para configurar o pipeline com plugins e sistemas.
    pub fn builder() -> PipelineBuilder {
        PipelineBuilder::default()
    }

    /// Executa um único tick: roda todos os stages em ordem.
    ///
    /// Retorna `Err` se algum sistema falhar de forma irrecuperável.
    pub fn tick(&mut self) -> Result<()> {
        debug!("pipeline tick: InputStage");
        self.input_schedule.run(&mut self.world);

        debug!("pipeline tick: PreProcessStage");
        self.pre_process_schedule.run(&mut self.world);

        debug!("pipeline tick: ProcessStage");
        self.process_schedule.run(&mut self.world);

        debug!("pipeline tick: PostProcessStage");
        self.post_process_schedule.run(&mut self.world);

        debug!("pipeline tick: OutputStage");
        self.output_schedule.run(&mut self.world);

        Ok(())
    }

    /// Loop principal: executa ticks até que um sistema de saída sinalize parada.
    ///
    /// A condição de parada é controlada pelo recurso [`PipelineState`].
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
    pub fn world(&self) -> &World { &self.world }
    pub fn world_mut(&mut self) -> &mut World { &mut self.world }
}

// ── PipelineBuilder ────────────────────────────────────────────────────────────

/// Builder fluente para configurar um [`Pipeline`].
#[derive(Default)]
pub struct PipelineBuilder {
    plugins: Vec<Box<dyn Plugin>>,
    input_schedule: Schedule,
    pre_process_schedule: Schedule,
    process_schedule: Schedule,
    post_process_schedule: Schedule,
    output_schedule: Schedule,
}

impl PipelineBuilder {
    /// Registra um plugin no pipeline.
    ///
    /// O plugin receberá `&mut self` do builder e poderá adicionar sistemas.
    /// Plugins duplicados (mesmo `name()`) são ignorados com aviso.
    pub fn add_plugin<P: Plugin>(mut self, plugin: P) -> Self {
        plugin.build(&mut self);
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Registra um sistema no `InputStage`.
    pub fn add_input_system<M>(
        &mut self,
        system: impl IntoSystemConfigs<M>,
    ) -> &mut Self {
        self.input_schedule.add_systems(system);
        self
    }

    /// Registra um sistema no `PreProcessStage`.
    pub fn add_pre_process_system<M>(
        &mut self,
        system: impl IntoSystemConfigs<M>,
    ) -> &mut Self {
        self.pre_process_schedule.add_systems(system);
        self
    }

    /// Registra um sistema no `ProcessStage`.
    pub fn add_process_system<M>(
        &mut self,
        system: impl IntoSystemConfigs<M>,
    ) -> &mut Self {
        self.process_schedule.add_systems(system);
        self
    }

    /// Registra um sistema no `PostProcessStage`.
    pub fn add_post_process_system<M>(
        &mut self,
        system: impl IntoSystemConfigs<M>,
    ) -> &mut Self {
        self.post_process_schedule.add_systems(system);
        self
    }

    /// Registra um sistema no `OutputStage`.
    pub fn add_output_system<M>(
        &mut self,
        system: impl IntoSystemConfigs<M>,
    ) -> &mut Self {
        self.output_schedule.add_systems(system);
        self
    }

    /// Constrói o [`Pipeline`] a partir da configuração atual.
    pub fn build(mut self) -> Pipeline {
        let mut world = World::new();

        // Inicializa recurso de estado do pipeline
        world.insert_resource(PipelineState::default());

        // Permite que plugins façam ajustes finais após todos serem registrados
        let plugins = std::mem::take(&mut self.plugins);
        for plugin in &plugins {
            plugin.finish(&mut self);
        }

        Pipeline {
            world,
            input_schedule: self.input_schedule,
            pre_process_schedule: self.pre_process_schedule,
            process_schedule: self.process_schedule,
            post_process_schedule: self.post_process_schedule,
            output_schedule: self.output_schedule,
        }
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
