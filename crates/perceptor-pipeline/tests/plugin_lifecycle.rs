//! Contrato do trait `Plugin`: duplicatas, dependências e cleanup.

use std::sync::{Arc, Mutex};

use bevy_ecs::prelude::*;
use perceptor_core::Error;
use perceptor_pipeline::{Pipeline, PipelineBuilder, PipelineState, Plugin};

type Log = Arc<Mutex<Vec<String>>>;

/// Plugin que registra cada chamada do ciclo de vida em um log compartilhado.
struct Probe {
    name: &'static str,
    deps: &'static [&'static str],
    log: Log,
}

impl Probe {
    fn new(name: &'static str, log: &Log) -> Self {
        Self::with_deps(name, &[], log)
    }

    fn with_deps(name: &'static str, deps: &'static [&'static str], log: &Log) -> Self {
        Self {
            name,
            deps,
            log: Arc::clone(log),
        }
    }

    fn record(&self, event: &str) {
        self.log
            .lock()
            .unwrap()
            .push(format!("{event} {}", self.name));
    }
}

impl Plugin for Probe {
    fn name(&self) -> &str {
        self.name
    }

    fn dependencies(&self) -> &[&str] {
        self.deps
    }

    fn build(&self, _builder: &mut PipelineBuilder) {
        self.record("build");
    }

    fn finish(&self, _builder: &mut PipelineBuilder) {
        self.record("finish");
    }

    fn cleanup(&self, world: &mut World) {
        let ticks = world.resource::<PipelineState>().tick_count;
        self.record(&format!("cleanup@{ticks}"));
    }
}

fn events(log: &Log) -> Vec<String> {
    log.lock().unwrap().clone()
}

#[test]
fn duplicate_plugin_is_rejected_and_not_built_twice() {
    let log = Log::default();
    let result = Pipeline::builder()
        .add_plugin(Probe::new("a", &log))
        .add_plugin(Probe::new("a", &log))
        .build();

    assert!(matches!(result, Err(Error::DuplicatePlugin { name }) if name == "a"));
    assert_eq!(
        events(&log),
        ["build a"],
        "sem segundo build, finish ou cleanup"
    );
}

#[test]
fn missing_dependency_is_rejected_at_build() {
    let log = Log::default();
    let result = Pipeline::builder()
        .add_plugin(Probe::with_deps("consumer", &["provider"], &log))
        .build();

    assert!(matches!(
        result,
        Err(Error::MissingPluginDependency { plugin, dependency })
            if plugin == "consumer" && dependency == "provider"
    ));
    assert_eq!(
        events(&log),
        ["build consumer"],
        "finish não roda em build inválido"
    );
}

#[test]
fn dependency_may_be_registered_after_the_dependent() {
    let log = Log::default();
    let pipeline = Pipeline::builder()
        .add_plugin(Probe::with_deps("consumer", &["provider"], &log))
        .add_plugin(Probe::new("provider", &log))
        .build();

    assert!(pipeline.is_ok());
}

#[test]
fn cleanup_runs_on_drop_in_reverse_registration_order() {
    let log = Log::default();
    let mut pipeline = Pipeline::builder()
        .add_plugin(Probe::new("a", &log))
        .add_plugin(Probe::new("b", &log))
        .add_plugin(Probe::new("c", &log))
        .build()
        .unwrap();
    pipeline.tick().unwrap();
    pipeline.tick().unwrap();
    log.lock().unwrap().clear();

    drop(pipeline);

    // O `World` ainda está íntegro durante o cleanup: dois ticks contados.
    assert_eq!(events(&log), ["cleanup@2 c", "cleanup@2 b", "cleanup@2 a"]);
}

#[test]
fn shutdown_runs_cleanup_once_even_if_called_again_or_dropped() {
    let log = Log::default();
    let mut pipeline = Pipeline::builder()
        .add_plugin(Probe::new("a", &log))
        .build()
        .unwrap();
    log.lock().unwrap().clear();

    pipeline.shutdown();
    pipeline.shutdown();
    drop(pipeline);

    assert_eq!(events(&log), ["cleanup@0 a"]);
}

#[test]
fn lifecycle_order_is_build_then_finish_per_registration_order() {
    let log = Log::default();
    let pipeline = Pipeline::builder()
        .add_plugin(Probe::new("a", &log))
        .add_plugin(Probe::new("b", &log))
        .build()
        .unwrap();

    assert_eq!(events(&log), ["build a", "build b", "finish a", "finish b"]);
    drop(pipeline);
}
