//! # perceptor-core
//!
//! Tipos fundamentais do Perceptor, sem dependência de ECS ou I/O.
//!
//! Esta crate é a base das demais (`perceptor-imgproc`, `perceptor-pipeline`)
//! e não admite `unsafe`.

#![forbid(unsafe_code)]

mod error;

pub use error::{BoxError, Error, Result};
