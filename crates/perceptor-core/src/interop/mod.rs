//! Conversões de e para outras crates do ecossistema, cada uma atrás de uma feature.

#[cfg(feature = "image")]
mod image;
#[cfg(feature = "ndarray")]
mod ndarray;

#[cfg(feature = "image")]
pub use self::image::ImagePixel;
