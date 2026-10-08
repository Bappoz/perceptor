//! Componente `Frame`: unidade fundamental de dados no pipeline.
//!
//! Um [`Frame`] é um tensor `[H, W, C]` de bytes associado a metadados
//! temporais. Ele é a **Entidade** central do ECS — sistemas recebem
//! queries sobre `Frame` para aplicar transformações.

use bevy_ecs::prelude::Component;
use image::DynamicImage;
use ndarray::Array3;

/// Metadados de um frame de vídeo/imagem.
#[derive(Debug, Clone)]
pub struct FrameMeta {
    /// Índice sequencial no stream (0-based).
    pub index: u64,
    /// Timestamp em microssegundos desde o início do stream.
    pub timestamp_us: u64,
    /// Fonte de origem (nome do arquivo, device ID, URL…).
    pub source: String,
}

/// Componente principal: frame de vídeo representado como tensor `[H, W, C]`.
///
/// # Representação dos dados
/// - Eixo 0 → altura (rows)
/// - Eixo 1 → largura (cols)
/// - Eixo 2 → canais (RGB = 3, RGBA = 4, L = 1)
///
/// # Invariante
/// `data.shape() == [height, width, channels]` **sempre** se mantém.
#[derive(Component, Debug)]
pub struct Frame {
    /// Metadados do frame.
    pub meta: FrameMeta,
    /// Tensor de pixels `[H, W, C]`, valores `u8` em `[0, 255]`.
    pub data: Array3<u8>,
}

impl Frame {
    /// Cria um frame a partir de tensor já construído.
    pub fn new(meta: FrameMeta, data: Array3<u8>) -> Self {
        debug_assert_eq!(
            data.ndim(),
            3,
            "Frame::new: esperado tensor 3D [H, W, C], recebido ndim={}",
            data.ndim()
        );
        Self { meta, data }
    }

    /// Converte um [`DynamicImage`] da crate `image` em [`Frame`].
    ///
    /// A imagem é sempre convertida para RGB8 (3 canais).
    /// Para preservar o canal alpha, use [`Frame::from_dynamic_image_rgba`].
    pub fn from_dynamic_image(meta: FrameMeta, img: DynamicImage) -> Self {
        let rgb = img.into_rgb8();
        let (w, h) = rgb.dimensions();
        let data = Array3::from_shape_vec(
            (h as usize, w as usize, 3),
            rgb.into_raw(),
        )
        .expect("conversão DynamicImage→Array3 falhou: shape inválido");
        Self::new(meta, data)
    }

    /// Converte um [`DynamicImage`] preservando canal alpha (RGBA, 4 canais).
    pub fn from_dynamic_image_rgba(meta: FrameMeta, img: DynamicImage) -> Self {
        let rgba = img.into_rgba8();
        let (w, h) = rgba.dimensions();
        let data = Array3::from_shape_vec(
            (h as usize, w as usize, 4),
            rgba.into_raw(),
        )
        .expect("conversão DynamicImage→Array3 RGBA falhou");
        Self::new(meta, data)
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    pub fn height(&self) -> usize   { self.data.shape()[0] }
    pub fn width(&self) -> usize    { self.data.shape()[1] }
    pub fn channels(&self) -> usize { self.data.shape()[2] }

    /// Retorna `true` se o frame é grayscale (1 canal).
    pub fn is_grayscale(&self) -> bool { self.channels() == 1 }
}
