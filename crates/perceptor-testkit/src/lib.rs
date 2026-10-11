//! # perceptor-testkit
//!
//! Infraestrutura de testes dos kernels do Perceptor. Crate interna
//! (`publish = false`): entra como `dev-dependency` de `perceptor-imgproc`,
//! `perceptor-pipeline` e da fachada.
//!
//! | Módulo | Para quê |
//! |---|---|
//! | [`patterns`] | imagens sintéticas determinísticas (gradiente, xadrez, impulso, ruído) |
//! | [`strategies`] | estratégias `proptest` de imagens |
//! | [`compare`] | diferença máxima, MSE e PSNR entre imagens |
//! | [`golden`] | imagens de referência em PNG, com receita de regeneração |
//! | [`fuzz`] | corpo dos alvos de fuzzing de `perceptor-core`, rodável também em `cargo test` |
//!
//! E duas macros: [`assert_images_eq!`] e [`assert_golden!`].
//!
//! `perceptor-core` **não** pode usar esta crate nos próprios testes: o
//! `dev-dependency` circular compilaria duas cópias de `perceptor-core` e
//! os tipos `Image<P>` de cada uma seriam incompatíveis.

pub mod compare;
pub mod fuzz;
pub mod golden;
pub mod patterns;
pub mod strategies;

mod sample;

pub use compare::{Diff, Mismatch, ShapeMismatch};
#[doc(hidden)]
pub use golden::__check_golden;
