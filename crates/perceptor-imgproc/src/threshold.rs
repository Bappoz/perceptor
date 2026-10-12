//! Limiarização global: um mesmo limiar para a imagem inteira.
//!
//! [`threshold`] compara cada pixel com um limiar `t` e decide o que fica,
//! conforme o [`ThresholdType`]. A comparação é sempre `v > t`, estrita.
//!
//! | Tipo | `v > t` | caso contrário |
//! |---|---|---|
//! | [`Binary`](ThresholdType::Binary) | `max` | `0` |
//! | [`BinaryInv`](ThresholdType::BinaryInv) | `0` | `max` |
//! | [`Trunc`](ThresholdType::Trunc) | `t` | `v` |
//! | [`ToZero`](ThresholdType::ToZero) | `v` | `0` |
//! | [`ToZeroInv`](ThresholdType::ToZeroInv) | `0` | `v` |
//!
//! # Otsu
//!
//! [`otsu_level`] escolhe o limiar a partir do histograma: o que divide os
//! pixels em duas classes com a maior **variância entre classes**,
//!
//! ```text
//! σ²(t) = w0(t) · w1(t) · (μ0(t) − μ1(t))²
//! ```
//!
//! onde `w` é a fração de pixels de cada classe e `μ` a média dela. É o
//! mesmo que minimizar a variância dentro das classes. Funciona quando o
//! histograma tem dois modos; em uma imagem com iluminação desigual, nenhum
//! limiar global serve.

use perceptor_core::{Image, ImageView, ImageViewMut, Result, L8};

use crate::map_pixels;

/// O que [`threshold`] faz com os pixels acima e abaixo do limiar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThresholdType {
    /// `max` acima do limiar, `0` nos demais. É o padrão.
    #[default]
    Binary,
    /// `0` acima do limiar, `max` nos demais.
    BinaryInv,
    /// Acima do limiar vira o próprio limiar; os demais ficam como estão.
    Trunc,
    /// Mantém os pixels acima do limiar e zera os demais.
    ToZero,
    /// Zera os pixels acima do limiar e mantém os demais.
    ToZeroInv,
}

impl ThresholdType {
    /// Aplica o tipo a um nível. `max` só é usado pelos tipos binários.
    #[must_use]
    pub const fn apply(self, value: u8, thresh: u8, max: u8) -> u8 {
        let above = value > thresh;
        match self {
            Self::Binary => {
                if above {
                    max
                } else {
                    0
                }
            }
            Self::BinaryInv => {
                if above {
                    0
                } else {
                    max
                }
            }
            Self::Trunc => {
                if above {
                    thresh
                } else {
                    value
                }
            }
            Self::ToZero => {
                if above {
                    value
                } else {
                    0
                }
            }
            Self::ToZeroInv => {
                if above {
                    0
                } else {
                    value
                }
            }
        }
    }
}

/// Limiariza a imagem em uma imagem nova.
///
/// `max` é o valor dos pixels "ligados" nos tipos binários; os demais tipos
/// o ignoram.
///
/// # Errors
/// [`Error::Allocation`](perceptor_core::Error::Allocation) se a saída não
/// puder ser alocada.
pub fn threshold(
    input: ImageView<'_, L8>,
    thresh: u8,
    max: u8,
    kind: ThresholdType,
) -> Result<Image<L8>> {
    map_pixels(input, |p| L8::new(kind.apply(p.luma(), thresh, max)))
}

/// Limiariza a imagem no lugar, sem alocar.
pub fn threshold_in_place(
    mut image: ImageViewMut<'_, L8>,
    thresh: u8,
    max: u8,
    kind: ThresholdType,
) {
    for pixel in image.pixels_mut() {
        *pixel = L8::new(kind.apply(pixel.luma(), thresh, max));
    }
}

/// Limiar de Otsu de uma imagem.
///
/// Equivale a [`otsu_from_histogram`] sobre o histograma da imagem.
#[must_use]
pub fn otsu_level(input: ImageView<'_, L8>) -> u8 {
    let mut histogram = [0u64; 256];
    for row in input.rows() {
        for pixel in row {
            histogram[usize::from(pixel.luma())] += 1;
        }
    }
    otsu_from_histogram(&histogram)
}

/// Limiar de Otsu a partir de um histograma de 256 níveis.
///
/// Devolve o `t` que maximiza a variância entre as classes `v ≤ t` e `v > t`.
/// Casos em que isso não define um único valor:
///
/// - **vários limiares empatados** (um vão sem pixels entre os dois modos):
///   o do meio do empate, para ficar longe dos dois modos;
/// - **um só nível presente**: esse nível — nada é maior que ele, e a saída
///   binária fica toda em `0`;
/// - **histograma vazio**: `0`.
#[must_use]
pub fn otsu_from_histogram(histogram: &[u64; 256]) -> u8 {
    // Somas exatas em inteiro; só a variância, que é comparada, vai a `f64`.
    let total: u64 = histogram.iter().sum();
    let weighted: u64 = (0u64..).zip(histogram).map(|(level, n)| level * n).sum();

    let mut below = 0u64; // pixels com v ≤ t
    let mut below_sum = 0u64; // soma dos níveis deles
    let mut best = 0.0f64;
    let mut plateau: Option<(u8, u8)> = None;
    for (level, &count) in (0u8..=255).zip(histogram) {
        below += count;
        below_sum += u64::from(level) * count;
        let above = total - below;
        if below == 0 || above == 0 {
            continue;
        }
        #[allow(clippy::cast_precision_loss)] // contagens de pixels, muito abaixo de 2^53
        let (w0, w1, s0, s1) = (
            below as f64,
            above as f64,
            below_sum as f64,
            (weighted - below_sum) as f64,
        );
        let gap = s0 / w0 - s1 / w1;
        let variance = w0 * w1 * gap * gap;
        // Em um vão sem pixels as somas não mudam, então os empates são
        // bit a bit iguais e a comparação exata os reconhece.
        #[allow(clippy::float_cmp)]
        match plateau {
            Some((first, _)) if variance == best => plateau = Some((first, level)),
            _ if variance > best || plateau.is_none() => {
                best = variance;
                plateau = Some((level, level));
            }
            _ => {}
        }
    }

    match plateau {
        Some((first, last)) => first.midpoint(last),
        // Nenhum limiar separa duas classes: zero ou um nível presente.
        None => histogram
            .iter()
            .position(|&n| n > 0)
            .and_then(|level| u8::try_from(level).ok())
            .unwrap_or(0),
    }
}

/// Limiariza com o limiar de Otsu e devolve também o limiar usado.
///
/// # Errors
/// [`Error::Allocation`](perceptor_core::Error::Allocation) se a saída não
/// puder ser alocada.
pub fn threshold_otsu(
    input: ImageView<'_, L8>,
    max: u8,
    kind: ThresholdType,
) -> Result<(u8, Image<L8>)> {
    let level = otsu_level(input);
    Ok((level, threshold(input, level, max, kind)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn otsu_picks_the_better_of_two_unequal_splits() {
        // Três picos: 10 e 20 juntos, 200 longe. Separar em 20|200 ganha de 10|20.
        let mut histogram = [0u64; 256];
        histogram[10] = 100;
        histogram[20] = 100;
        histogram[200] = 100;
        let level = otsu_from_histogram(&histogram);
        assert!((20..200).contains(&level), "{level}");
        assert_eq!(level, 109, "meio do empate 20..=199");
    }

    #[test]
    fn otsu_handles_levels_at_both_ends() {
        let mut histogram = [0u64; 256];
        histogram[0] = 5;
        histogram[255] = 5;
        assert_eq!(otsu_from_histogram(&histogram), 127);
    }
}
