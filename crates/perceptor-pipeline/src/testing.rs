//! Harness headless para testar sistemas e plugins.
//!
//! [`TestPipeline`] embrulha um [`Pipeline`] e tira o boilerplate de um teste
//! de sistema: injeta frames a partir de imagens, roda ticks e verifica quais
//! componentes cada entidade tem.
//!
//! ```
//! use perceptor_core::{Image, PixelFormat, Rgb8};
//! use perceptor_pipeline::plugins::filters::grayscale::{grayscale_system, GrayscaleTag};
//! use perceptor_pipeline::testing::TestPipeline;
//! use perceptor_pipeline::Stage;
//!
//! let mut test = TestPipeline::with_systems(Stage::Process, grayscale_system);
//! let frame = test.spawn(Image::<Rgb8>::new(4, 4)?);
//!
//! test.tick()?;
//!
//! test.assert_has::<GrayscaleTag>(frame);
//! assert_eq!(test.frame(frame).format(), PixelFormat::L8);
//! # Ok::<(), perceptor_core::Error>(())
//! ```
//!
//! Os métodos de verificação entram em pânico com uma mensagem que aponta
//! para a linha do teste (`#[track_caller]`); é o comportamento esperado de
//! um `assert`, e por isso este módulo não segue a regra de devolver `Result`.

use std::any::type_name;

use bevy_ecs::prelude::*;
use bevy_ecs::system::ScheduleSystem;
use perceptor_core::{Image, Result};

use crate::errors::PipelineErrors;
use crate::frame::{Frame, FrameMeta, FramePixel};
use crate::pipeline::{Pipeline, PipelineBuilder};
use crate::plugin::Plugin;
use crate::schedule::Stage;

/// Fonte registrada nos metadados dos frames criados por [`TestPipeline::spawn`].
pub const TEST_SOURCE: &str = "test";

/// Pipeline de teste: sem I/O, com frames injetados à mão.
pub struct TestPipeline {
    pipeline: Pipeline,
    next_index: u64,
}

impl Default for TestPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl TestPipeline {
    /// Pipeline vazio, sem sistemas.
    ///
    /// # Panics
    /// Não ocorre: um builder sem plugins não falha.
    #[must_use]
    #[track_caller]
    pub fn new() -> Self {
        Self::from_builder(Pipeline::builder())
    }

    /// Pipeline com os sistemas dados registrados em `stage`.
    ///
    /// Aceita o mesmo que `PipelineBuilder::add_system`: um sistema, uma
    /// tupla, ou `(a, b).chain()`.
    ///
    /// # Panics
    /// Não ocorre: um builder sem plugins não falha.
    #[must_use]
    #[track_caller]
    pub fn with_systems<M>(
        stage: Stage,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> Self {
        let mut builder = Pipeline::builder();
        builder.add_system(stage, systems);
        Self::from_builder(builder)
    }

    /// Pipeline com um único plugin.
    ///
    /// # Panics
    /// Se o plugin declarar uma dependência: use [`from_builder`](Self::from_builder)
    /// para registrar os demais.
    #[must_use]
    #[track_caller]
    pub fn with_plugin(plugin: impl Plugin) -> Self {
        Self::from_builder(Pipeline::builder().add_plugin(plugin))
    }

    /// Pipeline a partir de um builder já configurado (política de erro,
    /// vários plugins, recursos).
    ///
    /// # Panics
    /// Se `builder.build()` falhar (plugin duplicado ou dependência ausente).
    #[must_use]
    #[track_caller]
    pub fn from_builder(builder: PipelineBuilder) -> Self {
        match builder.build() {
            Ok(pipeline) => Self {
                pipeline,
                next_index: 0,
            },
            Err(error) => panic!("TestPipeline: o builder não pôde ser construído: {error}"),
        }
    }

    /// Injeta `image` como um frame e devolve a entidade.
    ///
    /// Os metadados têm fonte [`TEST_SOURCE`] e índice sequencial a partir de 0.
    pub fn spawn<P: FramePixel>(&mut self, image: Image<P>) -> Entity
    where
        Image<P>: Into<crate::frame::FrameData>,
    {
        let meta = FrameMeta::new(self.next_index, TEST_SOURCE);
        self.next_index += 1;
        self.spawn_frame(Frame::new(meta, image))
    }

    /// Injeta um frame com metadados escolhidos pelo teste.
    pub fn spawn_frame(&mut self, frame: Frame) -> Entity {
        self.pipeline.world_mut().spawn(frame).id()
    }

    /// Roda um tick.
    ///
    /// # Errors
    /// O erro de [`Pipeline::tick`], conforme a política do builder.
    pub fn tick(&mut self) -> Result<()> {
        self.pipeline.tick()
    }

    /// Roda `n` ticks, parando no primeiro erro.
    ///
    /// # Errors
    /// O erro de [`Pipeline::tick`], conforme a política do builder.
    pub fn run_ticks(&mut self, n: usize) -> Result<()> {
        (0..n).try_for_each(|_| self.tick())
    }

    /// Entidades com [`Frame`] ainda vivas, por índice do frame.
    pub fn frames(&mut self) -> Vec<Entity> {
        let world = self.pipeline.world_mut();
        let mut frames: Vec<(u64, Entity)> = world
            .query::<(Entity, &Frame)>()
            .iter(world)
            .map(|(entity, frame)| (frame.meta.index, entity))
            .collect();
        frames.sort_unstable();
        frames.into_iter().map(|(_, entity)| entity).collect()
    }

    /// Componente `C` de `entity`, se a entidade existir e o tiver.
    #[must_use]
    pub fn component<C: Component>(&self, entity: Entity) -> Option<&C> {
        self.pipeline.world().get::<C>(entity)
    }

    /// O [`Frame`] de `entity`.
    ///
    /// # Panics
    /// Se a entidade não existir ou não tiver `Frame`.
    #[must_use]
    #[track_caller]
    pub fn frame(&self, entity: Entity) -> &Frame {
        match self.component::<Frame>(entity) {
            Some(frame) => frame,
            None => panic!("{entity} não tem Frame (foi despawnada?)"),
        }
    }

    /// A imagem do frame de `entity`, no formato `P`.
    ///
    /// # Panics
    /// Se a entidade não tiver `Frame`, ou se o frame estiver em outro formato.
    #[must_use]
    #[track_caller]
    pub fn image<P: FramePixel>(&self, entity: Entity) -> &Image<P> {
        match self.frame(entity).as_image::<P>() {
            Ok(image) => image,
            Err(error) => panic!("frame de {entity}: {error:?}"),
        }
    }

    /// Verifica que `entity` tem o componente `C`.
    ///
    /// # Panics
    /// Se a entidade não existir ou não tiver `C`.
    #[track_caller]
    pub fn assert_has<C: Component>(&self, entity: Entity) {
        self.assert_has_all::<(C,)>(entity);
    }

    /// Verifica que `entity` não tem o componente `C`.
    ///
    /// # Panics
    /// Se a entidade não existir ou tiver `C`.
    #[track_caller]
    pub fn assert_lacks<C: Component>(&self, entity: Entity) {
        self.assert_lacks_all::<(C,)>(entity);
    }

    /// Verifica que `entity` tem todos os componentes da tupla `S`.
    ///
    /// # Panics
    /// Se a entidade não existir ou faltar algum; a mensagem lista os ausentes.
    #[track_caller]
    pub fn assert_has_all<S: ComponentSet>(&self, entity: Entity) {
        let missing = self.select::<S>(entity, false);
        assert!(
            missing.is_empty(),
            "{entity} deveria ter {}; faltam: {}",
            type_name::<S>(),
            missing.join(", ")
        );
    }

    /// Verifica que `entity` não tem nenhum dos componentes da tupla `S`.
    ///
    /// # Panics
    /// Se a entidade não existir ou tiver algum; a mensagem lista os presentes.
    #[track_caller]
    pub fn assert_lacks_all<S: ComponentSet>(&self, entity: Entity) {
        let present = self.select::<S>(entity, true);
        assert!(
            present.is_empty(),
            "{entity} não deveria ter nenhum de {}; presentes: {}",
            type_name::<S>(),
            present.join(", ")
        );
    }

    /// Verifica que `entity` ainda existe.
    ///
    /// # Panics
    /// Se a entidade tiver sido despawnada.
    #[track_caller]
    pub fn assert_alive(&self, entity: Entity) {
        assert!(
            self.pipeline.world().get_entity(entity).is_ok(),
            "{entity} foi despawnada"
        );
    }

    /// Verifica que `entity` foi despawnada.
    ///
    /// # Panics
    /// Se a entidade ainda existir.
    #[track_caller]
    pub fn assert_despawned(&self, entity: Entity) {
        assert!(
            self.pipeline.world().get_entity(entity).is_err(),
            "{entity} ainda existe"
        );
    }

    /// Erros reportados pelos sistemas no tick mais recente.
    #[must_use]
    pub fn errors(&self) -> &PipelineErrors {
        self.pipeline.errors()
    }

    /// O `World`, para inserir recursos ou consultar o que o harness não cobre.
    pub fn world_mut(&mut self) -> &mut World {
        self.pipeline.world_mut()
    }

    /// O pipeline embrulhado.
    pub fn pipeline(&mut self) -> &mut Pipeline {
        &mut self.pipeline
    }

    /// Nomes dos componentes de `S` cuja presença em `entity` é igual a `present`.
    #[track_caller]
    fn select<S: ComponentSet>(&self, entity: Entity, present: bool) -> Vec<&'static str> {
        let Ok(entity_ref) = self.pipeline.world().get_entity(entity) else {
            panic!("{entity} não existe (foi despawnada?)");
        };
        S::presence(entity_ref)
            .into_iter()
            .filter(|(_, has)| *has == present)
            .map(|(name, _)| name)
            .collect()
    }
}

/// Tupla de componentes aceita por [`TestPipeline::assert_has_all`] e
/// [`TestPipeline::assert_lacks_all`]. Implementada para tuplas de 1 a 6.
pub trait ComponentSet {
    /// Nome de cada componente e se `entity` o tem.
    #[doc(hidden)]
    fn presence(entity: EntityRef<'_>) -> Vec<(&'static str, bool)>;
}

macro_rules! component_set {
    ($($component:ident),+) => {
        impl<$($component: Component),+> ComponentSet for ($($component,)+) {
            fn presence(entity: EntityRef<'_>) -> Vec<(&'static str, bool)> {
                vec![$((type_name::<$component>(), entity.contains::<$component>())),+]
            }
        }
    };
}

component_set!(A);
component_set!(A, B);
component_set!(A, B, C);
component_set!(A, B, C, D);
component_set!(A, B, C, D, E);
component_set!(A, B, C, D, E, F);
