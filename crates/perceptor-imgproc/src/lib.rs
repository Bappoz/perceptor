//! # perceptor-imgproc
//!
//! Kernels de processamento de imagem do Perceptor como **funções puras**:
//! recebem e devolvem buffers, sem ECS, sem I/O e sem estado global.
//!
//! Isso permite testar, medir e otimizar cada algoritmo isoladamente; a
//! integração com o pipeline fica em `perceptor-pipeline`.

pub mod grayscale;
pub mod sobel;

/// Converte `f32` para `u8` com a semântica de `as`: satura em `[0, 255]` e trunca a fração.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn f32_to_u8(v: f32) -> u8 {
    v as u8
}
