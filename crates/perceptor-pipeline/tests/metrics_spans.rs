//! Spans `tracing` do pipeline: `tick` e, dentro dele, um `stage` por stage.
//!
//! Fica em um binário de teste próprio, com um único teste: o `tracing`
//! guarda por callsite se algum subscriber se interessa por ele, e um teste
//! paralelo sem subscriber deixaria os spans desligados para este.
#![cfg(feature = "metrics")]

use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use perceptor_pipeline::{Pipeline, Stage};

/// Subscriber mínimo: registra `enter`/`exit` de cada span pelo nome e campos.
#[derive(Default)]
struct Recorder {
    names: Mutex<Vec<String>>,
    events: Arc<Mutex<Vec<String>>>,
}

struct FieldText(String);

impl tracing::field::Visit for FieldText {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        // Escrever em `String` não falha.
        let _ = write!(self.0, " {}={value:?}", field.name());
    }
}

impl tracing::Subscriber for Recorder {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, attrs: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        let mut text = FieldText(attrs.metadata().name().to_owned());
        attrs.record(&mut text);
        let mut names = self.names.lock().unwrap();
        names.push(text.0);
        tracing::span::Id::from_u64(names.len() as u64)
    }

    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, _: &tracing::Event<'_>) {}

    fn enter(&self, id: &tracing::span::Id) {
        self.log("enter", id);
    }

    fn exit(&self, id: &tracing::span::Id) {
        self.log("exit", id);
    }
}

impl Recorder {
    fn log(&self, what: &str, id: &tracing::span::Id) {
        let index = usize::try_from(id.into_u64()).unwrap() - 1;
        let name = self.names.lock().unwrap()[index].clone();
        self.events.lock().unwrap().push(format!("{what} {name}"));
    }
}

#[test]
fn each_stage_runs_inside_its_own_span_nested_in_the_tick_span() {
    let recorder = Recorder::default();
    let events = Arc::clone(&recorder.events);
    let mut builder = Pipeline::builder();
    builder.add_system(Stage::Process, || {
        tracing::info_span!("user_system").in_scope(|| {});
    });
    let mut pipeline = builder.build().unwrap();

    tracing::subscriber::with_default(recorder, || pipeline.tick().unwrap());

    let events = events.lock().unwrap().clone();
    let mut expected = vec!["enter tick tick=1".to_owned()];
    for stage in Stage::ALL {
        expected.push(format!("enter stage stage={stage:?}"));
        if stage == Stage::Process {
            expected.push("enter user_system".to_owned());
            expected.push("exit user_system".to_owned());
        }
        expected.push(format!("exit stage stage={stage:?}"));
    }
    expected.push("exit tick tick=1".to_owned());
    assert_eq!(events, expected);
}
