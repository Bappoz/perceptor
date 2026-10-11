//! Conversões entre subpixels e a escala numérica comum dos comparadores.

use perceptor_core::{SampleType, Subpixel};

/// Valor do subpixel como `f64`, na unidade nativa (`0..=255`, `0..=65535`, `0.0..=1.0`).
pub(crate) fn to_f64<S: Subpixel>(sample: S) -> f64 {
    match S::SAMPLE {
        SampleType::U8 => f64::from(bytemuck::cast::<S, u8>(sample)),
        SampleType::U16 => f64::from(bytemuck::cast::<S, u16>(sample)),
        SampleType::F32 => f64::from(bytemuck::cast::<S, f32>(sample)),
    }
}

/// Subpixel para uma fração `t` de `0.0..=1.0` da faixa nominal, com arredondamento.
pub(crate) fn from_unit<S: Subpixel>(t: f64) -> S {
    let t = t.clamp(0.0, 1.0);
    // Os `as` abaixo são seguros: `t` está em `0..=1` e a faixa cabe no tipo de destino.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    match S::SAMPLE {
        SampleType::U8 => bytemuck::cast((t * 255.0).round() as u8),
        SampleType::U16 => bytemuck::cast((t * 65535.0).round() as u16),
        SampleType::F32 => bytemuck::cast(t as f32),
    }
}

/// Valor de fundo de escala (`MAX`) como `f64`.
pub(crate) fn peak<S: Subpixel>() -> f64 {
    to_f64(S::MAX)
}
