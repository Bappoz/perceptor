//! # perceptor-imgproc
//!
//! Kernels de processamento de imagem do Perceptor como **funções puras**:
//! recebem e devolvem buffers, sem ECS, sem I/O e sem estado global.
//!
//! Isso permite testar, medir e otimizar cada algoritmo isoladamente; a
//! integração com o pipeline fica em `perceptor-pipeline`.

pub mod grayscale;
pub mod sobel;
