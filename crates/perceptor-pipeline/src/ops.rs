//! Kernels puros como sistemas: [`PipelineBuilder::add_op`](crate::PipelineBuilder::add_op).
//!
//! Um kernel de `perceptor-imgproc` é uma função sobre imagens; para rodar no
//! pipeline ele precisa de um sistema que consulte os frames, confira o
//! formato, chame a função e trate o erro. Esse sistema é sempre o mesmo, e
//! `add_op` o gera a partir da assinatura da função:
//!
//! | Assinatura | Efeito no frame |
//! |---|---|
//! | `Fn(ImageView<A>) -> Result<Image<B>>` | `frame.data` é substituído pela saída |
//! | `Fn(ImageViewMut<P>)` | os pixels são alterados no lugar, sem alocar |
//! | `Fn(ImageViewMut<P>) -> Result<()>` | idem, podendo falhar |
//!
//! ```
//! use perceptor_core::{Image, ImageViewMut, PixelFormat, Rgb8, L8};
//! use perceptor_imgproc::grayscale::convert_to_gray;
//! use perceptor_pipeline::testing::TestPipeline;
//! use perceptor_pipeline::{Pipeline, Stage};
//!
//! fn invert(mut image: ImageViewMut<'_, L8>) {
//!     for pixel in image.pixels_mut() {
//!         *pixel = L8::new(255 - pixel.luma());
//!     }
//! }
//!
//! let mut builder = Pipeline::builder();
//! builder
//!     .add_op(Stage::Process, convert_to_gray)
//!     .add_op(Stage::Process, invert);
//! let mut test = TestPipeline::from_builder(builder);
//! let frame = test.spawn(Image::<Rgb8>::new(4, 4)?);
//!
//! test.tick()?;
//!
//! assert_eq!(test.frame(frame).format(), PixelFormat::L8);
//! assert_eq!(test.image::<L8>(frame).row(0)[0], L8::new(255));
//! # Ok::<(), perceptor_core::Error>(())
//! ```
//!
//! Regras do sistema gerado:
//!
//! - só toca frames no formato de entrada da op; os demais passam intactos e
//!   sem erro;
//! - um `Err` vai para [`PipelineErrors`] como erro daquele frame, com o nome
//!   da função, e o frame fica como estava;
//! - ops do mesmo stage rodam na ordem em que foram registradas, de modo que
//!   cada uma vê o formato produzido pela anterior. Em relação a sistemas
//!   registrados com `add_system` no mesmo stage não há ordem.
//!
//! Sem marcação, a op roda de novo a cada tick sobre um frame que continue no
//! `World` e no formato de entrada. [`Op::tagged`] evita isso.

use std::any::type_name;
use std::marker::PhantomData;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleConfigs;
use bevy_ecs::system::ScheduleSystem;
use perceptor_core::{Image, ImageView, ImageViewMut, PixelFormat, Result};

use crate::errors::PipelineErrors;
use crate::frame::{Frame, FrameData, FramePixel};

/// Uma operação sobre o [`Frame`], implementada pelas funções com uma das
/// assinaturas listadas no [módulo](self).
///
/// `M` só distingue as assinaturas e é sempre inferido.
pub trait Op<M>: Send + Sync + Sized + 'static {
    /// Formato que o frame precisa ter para a op se aplicar.
    fn input_format(&self) -> PixelFormat;

    /// Aplica a op a `frame`.
    ///
    /// # Errors
    /// - [`FormatMismatch`](perceptor_core::Error::FormatMismatch) se o frame
    ///   não estiver em [`input_format`](Self::input_format);
    /// - o erro devolvido pela função. Nos dois casos o frame não muda.
    fn apply(&self, frame: &mut Frame) -> Result<()>;

    /// Marca cada frame processado com o componente `T` e passa a ignorar os
    /// que já o têm.
    ///
    /// O componente é inserido via `Commands`: a op seguinte do mesmo stage e
    /// os stages seguintes já o veem no mesmo tick. Um frame em que a op
    /// falhou não é marcado.
    fn tagged<T: Component + Default>(self) -> Tagged<Self, T, M> {
        Tagged {
            op: self,
            marker: PhantomData,
        }
    }
}

/// Marcador de [`Op`]: função `ImageView<A> -> Result<Image<B>>`.
#[doc(hidden)]
pub struct Map<A, B>(PhantomData<fn(A) -> B>);

/// Marcador de [`Op`]: função in-place que não falha.
#[doc(hidden)]
pub struct InPlace<P>(PhantomData<fn(P)>);

/// Marcador de [`Op`]: função in-place que retorna `Result<()>`.
#[doc(hidden)]
pub struct TryInPlace<P>(PhantomData<fn(P)>);

impl<A, B, F> Op<Map<A, B>> for F
where
    A: FramePixel,
    B: FramePixel,
    Image<B>: Into<FrameData>,
    F: for<'a> Fn(ImageView<'a, A>) -> Result<Image<B>> + Send + Sync + 'static,
{
    fn input_format(&self) -> PixelFormat {
        A::FORMAT
    }

    fn apply(&self, frame: &mut Frame) -> Result<()> {
        let output = self(frame.as_image::<A>()?.view())?;
        frame.data = output.into();
        Ok(())
    }
}

impl<P, F> Op<InPlace<P>> for F
where
    P: FramePixel,
    F: for<'a> Fn(ImageViewMut<'a, P>) + Send + Sync + 'static,
{
    fn input_format(&self) -> PixelFormat {
        P::FORMAT
    }

    fn apply(&self, frame: &mut Frame) -> Result<()> {
        self(frame.as_image_mut::<P>()?.view_mut());
        Ok(())
    }
}

impl<P, F> Op<TryInPlace<P>> for F
where
    P: FramePixel,
    F: for<'a> Fn(ImageViewMut<'a, P>) -> Result<()> + Send + Sync + 'static,
{
    fn input_format(&self) -> PixelFormat {
        P::FORMAT
    }

    fn apply(&self, frame: &mut Frame) -> Result<()> {
        self(frame.as_image_mut::<P>()?.view_mut())
    }
}

/// Uma [`Op`] que marca os frames processados com `T`. Criada por [`Op::tagged`].
pub struct Tagged<O, T, M> {
    op: O,
    marker: PhantomData<fn() -> (T, M)>,
}

/// O que [`PipelineBuilder::add_op`](crate::PipelineBuilder::add_op) aceita:
/// uma [`Op`], ou uma [`Op`] com marcação ([`Op::tagged`]).
pub trait IntoOpSystem<M> {
    /// Converte a op no sistema que a aplica a cada frame.
    fn into_op_system(self) -> ScheduleConfigs<ScheduleSystem>;
}

/// Marcador de [`IntoOpSystem`]: op sem marcação.
#[doc(hidden)]
pub struct Untagged<M>(PhantomData<fn() -> M>);

impl<M: 'static, O: Op<M>> IntoOpSystem<Untagged<M>> for O {
    fn into_op_system(self) -> ScheduleConfigs<ScheduleSystem> {
        let input = self.input_format();
        let system = move |mut frames: Query<(Entity, &mut Frame)>,
                           mut errors: ResMut<PipelineErrors>| {
            for (entity, mut frame) in &mut frames {
                // Comparar antes de `&mut`: um frame ignorado não deve
                // constar como alterado na detecção de mudanças.
                if frame.format() != input {
                    continue;
                }
                if let Err(error) = self.apply(&mut frame) {
                    errors.report_frame(type_name::<O>(), entity, error);
                }
            }
        };
        system.into_configs()
    }
}

impl<M: 'static, O: Op<M>, T: Component + Default> IntoOpSystem<Tagged<(), T, M>>
    for Tagged<O, T, M>
{
    fn into_op_system(self) -> ScheduleConfigs<ScheduleSystem> {
        let op = self.op;
        let input = op.input_format();
        let system = move |mut frames: Query<(Entity, &mut Frame), Without<T>>,
                           mut commands: Commands,
                           mut errors: ResMut<PipelineErrors>| {
            for (entity, mut frame) in &mut frames {
                if frame.format() != input {
                    continue;
                }
                match op.apply(&mut frame) {
                    Ok(()) => {
                        commands.entity(entity).insert(T::default());
                    }
                    Err(error) => errors.report_frame(type_name::<O>(), entity, error),
                }
            }
        };
        system.into_configs()
    }
}

/// Set de uma op: dá ao builder um alvo para ordenar a op seguinte do stage.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct OpSet(pub(crate) usize);
