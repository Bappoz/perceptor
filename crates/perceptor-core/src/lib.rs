//! # perceptor-core
//!
//! Tipos fundamentais do Perceptor, sem dependência de ECS ou I/O.
//!
//! Esta crate é a base das demais (`perceptor-imgproc`, `perceptor-pipeline`)
//! e não admite `unsafe`.

#![forbid(unsafe_code)]

mod error;
mod image;
mod iter;
mod pixel;
mod view;

pub use error::{BoxError, Error, Result};
pub use image::Image;
pub use pixel::{
    Bgr8, La8, Pixel, PixelFormat, Rgb8, RgbF32, Rgba8, SampleType, Subpixel, L16, L8, LF32,
};
pub use view::{ImageView, ImageViewMut, Rect};
