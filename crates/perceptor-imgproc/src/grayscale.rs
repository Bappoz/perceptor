//! Conversão para escala de cinza.
//!
//! # Algoritmo
//! Luminância como soma ponderada dos canais, com os pesos de
//! [`LumaWeights`]:
//!
//! ```text
//! Y = wr·R + wg·G + wb·B
//! ```
//!
//! A conta é feita em ponto fixo Q15: cada peso vira um inteiro
//! `round(w · 2¹⁵)`, ajustado para que os três somem exatamente `2¹⁵`, e
//!
//! ```text
//! Y = (Wr·R + Wg·G + Wb·B + 2¹⁴) >> 15
//! ```
//!
//! O `+ 2¹⁴` arredonda para o mais próximo. Como os pesos somam `2¹⁵`, um
//! pixel cinza `(v, v, v)` resulta exatamente em `v`. O resultado difere do
//! cálculo em `f64` ([`mod@reference`]) em no máximo um nível.
//!
//! Quinze bits é o máximo em que os pesos ainda cabem em `i16`, o que deixa
//! o laço aberto a multiplicações de 16 bits em SIMD.
//!
//! # Paralelismo
//! Imagens pequenas são convertidas na thread que chamou. Acima de
//! [`PARALLEL_THRESHOLD`] pixels a saída é dividida em faixas de linhas,
//! uma tarefa rayon por faixa.

use perceptor_core::{Image, ImageView, Result, Rgb8, L8};
use rayon::prelude::*;

/// Bits de fração dos pesos em ponto fixo.
const SHIFT: u32 = 15;
/// Metade de uma unidade em Q15: somada antes do deslocamento, arredonda.
const HALF: u32 = 1 << (SHIFT - 1);

/// Número de pixels a partir do qual a conversão usa mais de uma thread.
///
/// Abaixo disso, acordar o pool do rayon custa mais do que a conversão.
pub const PARALLEL_THRESHOLD: usize = 64 * 1024;
/// Pixels por faixa no caminho paralelo: grande o bastante para o custo de
/// agendar a tarefa não aparecer, pequeno o bastante para repartir o trabalho.
const PIXELS_PER_STRIP: usize = 32 * 1024;

/// Pesos de luminância: quanto cada canal contribui para o cinza.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LumaWeights {
    /// ITU-R BT.601 (SDTV, JPEG): `0.299·R + 0.587·G + 0.114·B`. É o padrão,
    /// e o que `OpenCV` e a maioria das bibliotecas usam para `RGB → cinza`.
    #[default]
    Bt601,
    /// ITU-R BT.709 (HDTV, sRGB): `0.2126·R + 0.7152·G + 0.0722·B`.
    Bt709,
}

impl LumaWeights {
    /// Os pesos `[R, G, B]` como frações; somam `1`.
    #[must_use]
    pub const fn coefficients(self) -> [f64; 3] {
        match self {
            Self::Bt601 => [0.299, 0.587, 0.114],
            Self::Bt709 => [0.2126, 0.7152, 0.0722],
        }
    }

    /// Os pesos `[R, G, B]` em Q15; somam exatamente `2¹⁵`.
    ///
    /// Cada um é `round(w · 2¹⁵)`, exceto o azul do BT.601: arredondar os
    /// três daria `2¹⁵ + 1`, e ele é o que está mais perto de `.5`
    /// (`3735.552`), portanto o que menos perde ao ir para baixo.
    #[must_use]
    pub const fn fixed(self) -> [u32; 3] {
        match self {
            Self::Bt601 => [9798, 19235, 3735],
            Self::Bt709 => [6966, 23436, 2366],
        }
    }
}

/// Luminância de um pixel em ponto fixo.
#[inline]
fn luma_fixed(pixel: Rgb8, [wr, wg, wb]: [u32; 3]) -> L8 {
    let sum =
        wr * u32::from(pixel.r()) + wg * u32::from(pixel.g()) + wb * u32::from(pixel.b()) + HALF;
    // Os pesos somam 2¹⁵, então `sum >> 15` é no máximo
    // `(255 · 2¹⁵ + 2¹⁴) >> 15 = 255`: a conversão nunca trunca.
    #[allow(clippy::cast_possible_truncation)]
    L8::new((sum >> SHIFT) as u8)
}

fn gray_row(dst: &mut [L8], src: &[Rgb8], weights: [u32; 3]) {
    for (out, pixel) in dst.iter_mut().zip(src) {
        *out = luma_fixed(*pixel, weights);
    }
}

/// Converte uma imagem RGB para luminância com os pesos BT.601.
///
/// Equivale a [`convert_to_gray_with`] com [`LumaWeights::Bt601`]. Aceita
/// qualquer visão, inclusive um recorte com stride maior que a linha.
///
/// # Errors
/// [`Error::Allocation`](perceptor_core::Error::Allocation) se a imagem de
/// saída não puder ser alocada.
pub fn convert_to_gray(input: ImageView<'_, Rgb8>) -> Result<Image<L8>> {
    convert_to_gray_with(input, LumaWeights::Bt601)
}

/// Converte uma imagem RGB para luminância com os pesos dados.
///
/// O resultado é arredondado para o nível mais próximo e fica a no máximo um
/// nível de [`reference::convert_to_gray`].
///
/// # Errors
/// [`Error::Allocation`](perceptor_core::Error::Allocation) se a imagem de
/// saída não puder ser alocada.
pub fn convert_to_gray_with(input: ImageView<'_, Rgb8>, weights: LumaWeights) -> Result<Image<L8>> {
    let (width, height) = (input.width(), input.height());
    let mut output = Image::<L8>::new(width, height)?;
    let fixed = weights.fixed();

    if width * height < PARALLEL_THRESHOLD {
        for (dst, src) in output.rows_mut().zip(input.rows()) {
            gray_row(dst, src, fixed);
        }
        return Ok(output);
    }

    let rows_per_strip = (PIXELS_PER_STRIP / width).max(1);
    output
        .view_mut()
        .par_strips_mut(rows_per_strip)
        .for_each(|(y0, mut strip)| {
            for (dy, dst) in strip.rows_mut().enumerate() {
                gray_row(dst, input.row(y0 + dy), fixed);
            }
        });
    Ok(output)
}

/// Versão escalar de referência: a definição contra a qual a otimizada é testada.
pub mod reference {
    use perceptor_core::{Image, ImageView, Result, Rgb8, L8};

    use super::LumaWeights;

    /// Converte para luminância em `f64`, um pixel por vez, arredondando
    /// para o nível mais próximo.
    ///
    /// Lenta de propósito: serve de oráculo nos testes e de base nos
    /// benchmarks, não para uso em pipeline.
    ///
    /// # Errors
    /// [`Error::Allocation`](perceptor_core::Error::Allocation) se a imagem de
    /// saída não puder ser alocada.
    pub fn convert_to_gray(input: ImageView<'_, Rgb8>, weights: LumaWeights) -> Result<Image<L8>> {
        let mut output = Image::<L8>::new(input.width(), input.height())?;
        for (dst, src) in output.rows_mut().zip(input.rows()) {
            for (out, pixel) in dst.iter_mut().zip(src) {
                *out = L8::new(luma(*pixel, weights));
            }
        }
        Ok(output)
    }

    /// Luminância de um pixel em `f64`, arredondada.
    #[must_use]
    pub fn luma(pixel: Rgb8, weights: LumaWeights) -> u8 {
        let [wr, wg, wb] = weights.coefficients();
        let y = wr * f64::from(pixel.r()) + wg * f64::from(pixel.g()) + wb * f64::from(pixel.b());
        // Os pesos somam 1 e os canais vão até 255: `y` está em `[0, 255]`.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let level = y.round() as u8;
        level
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perceptor_core::Rect;

    fn gray_of(pixel: Rgb8) -> u8 {
        let input = Image::filled(1, 1, pixel).unwrap();
        convert_to_gray(input.view()).unwrap().as_pixels()[0].luma()
    }

    #[test]
    fn white_pixel() {
        assert_eq!(gray_of(Rgb8::new(255, 255, 255)), 255);
    }

    #[test]
    fn black_pixel() {
        assert_eq!(gray_of(Rgb8::new(0, 0, 0)), 0);
    }

    #[test]
    fn pure_red() {
        // R=255, G=0, B=0 → Y = 0.299 * 255 ≈ 76
        assert_eq!(gray_of(Rgb8::new(255, 0, 0)), 76);
    }

    #[test]
    fn fixed_weights_sum_to_one_and_follow_the_coefficients() {
        for weights in [LumaWeights::Bt601, LumaWeights::Bt709] {
            let fixed = weights.fixed();
            assert_eq!(fixed.iter().sum::<u32>(), 1 << SHIFT, "{weights:?}");
            for (q15, w) in fixed.into_iter().zip(weights.coefficients()) {
                let exact = w * f64::from(1u32 << SHIFT);
                assert!(
                    (f64::from(q15) - exact).abs() < 1.0,
                    "{weights:?}: {q15} vs {exact}"
                );
            }
        }
    }

    /// Todos os 2²⁴ pixels, nos dois conjuntos de pesos: nunca mais de um
    /// nível de diferença para a referência em `f64`.
    #[test]
    fn every_rgb_value_is_within_one_level_of_the_reference() {
        for weights in [LumaWeights::Bt601, LumaWeights::Bt709] {
            let fixed = weights.fixed();
            let mut differing = 0u32;
            for rgb in 0..1u32 << 24 {
                let [_, r, g, b] = rgb.to_be_bytes();
                let pixel = Rgb8::new(r, g, b);
                let fast = luma_fixed(pixel, fixed).luma();
                let exact = reference::luma(pixel, weights);
                assert!(
                    fast.abs_diff(exact) <= 1,
                    "{weights:?} {pixel:?}: {fast} vs {exact}"
                );
                differing += u32::from(fast != exact);
            }
            println!(
                "{weights:?}: {differing} de {} valores diferem em 1 nível",
                1u32 << 24
            );
        }
    }

    #[test]
    fn output_shape() {
        let input = Image::filled(6, 4, Rgb8::new(128, 128, 128)).unwrap();
        let out = convert_to_gray(input.view()).unwrap();
        assert_eq!(out.dimensions(), (6, 4));
    }

    #[test]
    fn accepts_a_non_contiguous_roi() {
        // Coluna x vale (x, 0, 0): o recorte começa em x = 1 e tem stride maior que a linha.
        let input =
            Image::from_fn(4, 3, |x, _| Rgb8::new(u8::try_from(x).unwrap() * 80, 0, 0)).unwrap();
        let roi = input.roi(Rect::new(1, 1, 2, 2)).unwrap();
        assert!(!roi.is_contiguous());

        let out = convert_to_gray(roi).unwrap();

        assert_eq!(out.dimensions(), (2, 2));
        let expected = [gray_of(Rgb8::new(80, 0, 0)), gray_of(Rgb8::new(160, 0, 0))];
        for row in out.rows() {
            assert_eq!([row[0].luma(), row[1].luma()], expected);
        }
    }
}
