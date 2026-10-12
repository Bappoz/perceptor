//! # perceptor-imgproc
//!
//! Kernels de processamento de imagem do Perceptor como **funções puras**:
//! recebem e devolvem buffers, sem ECS, sem I/O e sem estado global.
//!
//! Isso permite testar, medir e otimizar cada algoritmo isoladamente; a
//! integração com o pipeline fica em `perceptor-pipeline`.

pub mod channels;
pub mod grayscale;
pub mod hsv;
pub mod sobel;

/// Converte `f32` para `u8` com a semântica de `as`: satura em `[0, 255]` e trunca a fração.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn f32_to_u8(v: f32) -> u8 {
    v as u8
}

/// Aplica `f` a cada pixel de `input`, em uma imagem nova.
pub(crate) fn map_pixels<A, B>(
    input: perceptor_core::ImageView<'_, A>,
    f: impl Fn(A) -> B,
) -> perceptor_core::Result<perceptor_core::Image<B>>
where
    A: perceptor_core::Pixel,
    B: perceptor_core::Pixel,
{
    let mut output = perceptor_core::Image::<B>::new(input.width(), input.height())?;
    for (dst, src) in output.rows_mut().zip(input.rows()) {
        for (out, pixel) in dst.iter_mut().zip(src) {
            *out = f(*pixel);
        }
    }
    Ok(output)
}
