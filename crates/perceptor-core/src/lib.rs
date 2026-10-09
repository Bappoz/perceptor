//! # perceptor-core
//!
//! Tipos fundamentais do Perceptor, sem dependência de ECS ou I/O.
//!
//! Esta crate é a base das demais (`perceptor-imgproc`, `perceptor-pipeline`)
//! e não admite `unsafe`.

#![forbid(unsafe_code)]

mod error;
mod image;
mod pixel;

pub use error::{BoxError, Error, Result};
pub use image::Image;
pub use pixel::{
    Bgr8, La8, Pixel, PixelFormat, Rgb8, RgbF32, Rgba8, SampleType, Subpixel, L16, L8, LF32,
};
