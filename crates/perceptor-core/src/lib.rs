//! # perceptor-core
//!
//! Tipos fundamentais do Perceptor, sem dependência de ECS ou I/O.
//!
//! Esta crate é a base das demais (`perceptor-imgproc`, `perceptor-pipeline`)
//! e não admite `unsafe`.

#![forbid(unsafe_code)]

mod border;
mod error;
mod image;
mod interop;
mod iter;
mod pixel;
mod pool;
mod view;

pub use border::BorderMode;
pub use error::{BoxError, Error, Result};
pub use image::Image;
#[cfg(feature = "image")]
pub use interop::ImagePixel;
pub use pixel::{
    Bgr8, Hsl8, HslF32, Hsv8, HsvF32, La8, Pixel, PixelFormat, Rgb8, RgbF32, Rgba8, SampleType,
    Subpixel, L16, L8, LF32,
};
pub use pool::{BufferPool, PoolStats, PooledImage};
pub use view::{ImageView, ImageViewMut, Rect};
