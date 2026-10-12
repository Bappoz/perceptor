//! Máscara por faixa de cor: `in_range`.
//!
//! Segmentar por cor é perguntar, pixel a pixel, "este pixel cai dentro da
//! faixa `[lower, upper]` em **todos** os canais?". A resposta é uma máscara
//! `L8` — `255` onde sim, `0` onde não — que alimenta as operações com
//! máscara de [`arith`](crate::arith) e de [`histogram`](crate::histogram).
//!
//! Os limites são **inclusivos** e dados como um pixel do mesmo formato:
//! `in_range(view, Rgb8::new(10, 20, 30), Rgb8::new(50, 60, 70))`.
//!
//! # Matiz que dá a volta
//!
//! O matiz é um ângulo: vermelho fica dos dois lados do zero. Em formatos
//! HSV/HSL (`Hsv8`, `Hsl8`, `HsvF32`, `HslF32`), uma faixa com `lower.h >
//! upper.h` é lida como a que **atravessa o zero**: casa quando `h ≥ lower.h`
//! **ou** `h ≤ upper.h`. Em `Hsv8` (matiz `0..180`), `170..=10` é o vermelho.
//!
//! Nos demais canais, e em qualquer canal de outros formatos, um limite
//! invertido não dá a volta: a faixa é vazia e nenhum pixel casa.

use perceptor_core::{Image, ImageView, Pixel, PixelFormat, Result, L8};

use crate::map_pixels;

/// `true` nos formatos cujo canal 0 é um matiz circular.
const fn has_hue(format: PixelFormat) -> bool {
    matches!(
        format,
        PixelFormat::Hsv8 | PixelFormat::Hsl8 | PixelFormat::HsvF32 | PixelFormat::HslF32
    )
}

/// Máscara dos pixels com `lower ≤ canal ≤ upper` em todos os canais.
///
/// Devolve `255` onde o pixel está na faixa e `0` fora dela. `NaN` em um
/// canal `f32` nunca está na faixa. Veja o módulo para a regra do matiz.
///
/// # Errors
/// [`Error::Allocation`](perceptor_core::Error::Allocation) se a saída não
/// puder ser alocada.
pub fn in_range<P: Pixel>(input: ImageView<'_, P>, lower: P, upper: P) -> Result<Image<L8>> {
    let (lo, hi) = (lower.channels(), upper.channels());
    let wraps = has_hue(P::FORMAT) && lo[0] > hi[0];
    map_pixels(input, |pixel| {
        let channels = pixel.channels();
        let hue_ok = if wraps {
            channels[0] >= lo[0] || channels[0] <= hi[0]
        } else {
            lo[0] <= channels[0] && channels[0] <= hi[0]
        };
        let inside = hue_ok
            && channels
                .iter()
                .zip(lo.iter().zip(hi))
                .skip(1)
                .all(|(v, (l, h))| l <= v && v <= h);
        L8::new(if inside { 255 } else { 0 })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use perceptor_core::{Hsv8, Rgb8};

    #[test]
    fn only_hue_formats_wrap() {
        assert!(has_hue(Hsv8::FORMAT));
        assert!(!has_hue(Rgb8::FORMAT));
    }
}
