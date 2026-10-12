//! Histograma de intensidade e equalização global.
//!
//! O histograma de um canal de 8 bits é a contagem de pixels em cada um dos
//! 256 níveis. Uma máscara opcional restringe a contagem aos pixels em que
//! ela é diferente de `0`, o que permite medir só uma região (a de interesse
//! em uma segmentação, por exemplo).
//!
//! # Equalização
//!
//! Equalizar redistribui os níveis para que o histograma fique o mais plano
//! possível, ocupando toda a faixa `0..=255`. A transformação é a função de
//! distribuição acumulada (CDF) do próprio histograma, reescalada:
//!
//! ```text
//!                cdf(v) − cdf_min
//! lut[v] = 255 · ──────────────────
//!                  N − cdf_min
//! ```
//!
//! onde `N` é o total de pixels e `cdf_min` a CDF do menor nível presente,
//! de modo que esse nível vai a `0` e o maior a `255`. É uma [`Lut`], então
//! preserva a ordem dos pixels: nunca troca qual é mais claro.
//!
//! Arredonda metade para cima. Um histograma com um só nível (ou vazio) não
//! tem contraste a redistribuir, e a tabela é a identidade.

use perceptor_core::{Error, Image, ImageView, Pixel, Result, L8};

use crate::lut::{apply_lut, Lut};

/// Contagem de pixels por nível de 8 bits.
pub type Histogram = [u64; 256];

/// Confere que a máscara tem as dimensões da imagem.
fn check_mask<P: Pixel>(input: &ImageView<'_, P>, mask: Option<&ImageView<'_, L8>>) -> Result<()> {
    match mask {
        Some(m) if m.dimensions() != input.dimensions() => Err(Error::ShapeMismatch {
            expected: input.dimensions(),
            actual: m.dimensions(),
        }),
        _ => Ok(()),
    }
}

/// Chama `f` para cada pixel contado: todos, ou os que a máscara liga.
fn for_each_counted<P: Pixel>(
    input: &ImageView<'_, P>,
    mask: Option<&ImageView<'_, L8>>,
    mut f: impl FnMut(&P),
) {
    for (y, row) in input.rows().enumerate() {
        match mask {
            None => row.iter().for_each(&mut f),
            Some(m) => {
                for (pixel, keep) in row.iter().zip(m.row(y)) {
                    if keep.luma() != 0 {
                        f(pixel);
                    }
                }
            }
        }
    }
}

/// Histograma de um canal.
///
/// `mask`, se presente, deve ter as dimensões de `input`; só os pixels em
/// que ela é diferente de `0` entram na contagem.
///
/// # Errors
/// - [`Error::ChannelOutOfRange`] se `channel` não for um canal do formato.
/// - [`Error::ShapeMismatch`] se a máscara tiver outras dimensões.
pub fn histogram<P>(
    input: ImageView<'_, P>,
    channel: usize,
    mask: Option<ImageView<'_, L8>>,
) -> Result<Histogram>
where
    P: Pixel<Subpixel = u8>,
{
    if channel >= P::CHANNELS {
        return Err(Error::ChannelOutOfRange {
            index: channel,
            channels: P::CHANNELS,
        });
    }
    check_mask(&input, mask.as_ref())?;
    let mut counts = [0u64; 256];
    for_each_counted(&input, mask.as_ref(), |pixel| {
        counts[usize::from(pixel.channels()[channel])] += 1;
    });
    Ok(counts)
}

/// Um histograma por canal, em uma só passada pela imagem.
///
/// # Errors
/// [`Error::ShapeMismatch`] se a máscara tiver outras dimensões.
pub fn histograms<P>(
    input: ImageView<'_, P>,
    mask: Option<ImageView<'_, L8>>,
) -> Result<Vec<Histogram>>
where
    P: Pixel<Subpixel = u8>,
{
    check_mask(&input, mask.as_ref())?;
    let mut counts = vec![[0u64; 256]; P::CHANNELS];
    for_each_counted(&input, mask.as_ref(), |pixel| {
        for (histogram, &value) in counts.iter_mut().zip(pixel.channels()) {
            histogram[usize::from(value)] += 1;
        }
    });
    Ok(counts)
}

/// Soma acumulada de um histograma: `cdf[v]` conta os pixels com nível `≤ v`.
#[must_use]
pub fn cumulative(histogram: &Histogram) -> Histogram {
    let mut total = 0u64;
    histogram.map(|count| {
        total += count;
        total
    })
}

/// Tabela de equalização de um histograma.
///
/// Aplicá-la com [`apply_lut`] a qualquer imagem equaliza essa imagem *como
/// se* tivesse o histograma dado, o que permite calcular o histograma sobre
/// uma máscara e equalizar a imagem inteira.
///
/// Níveis abaixo do primeiro nível presente vão a `0`. Histograma vazio ou
/// de um só nível: a identidade.
#[must_use]
pub fn equalization_lut(histogram: &Histogram) -> Lut<u8> {
    let total: u64 = histogram.iter().sum();
    let first = histogram.iter().find(|&&n| n > 0).copied();
    let Some(min) = first.filter(|&n| n < total) else {
        return Lut::identity();
    };
    let cdf = cumulative(histogram);
    let denominator = u128::from(total - min);
    Lut::from_fn(|level| {
        let above = cdf[usize::from(level)].saturating_sub(min);
        // `255 · above / denominator`, arredondado com metade para cima.
        let scaled = (u128::from(above) * 255 * 2 + denominator) / (2 * denominator);
        u8::try_from(scaled).unwrap_or(u8::MAX)
    })
}

/// Equaliza o histograma de uma imagem em tons de cinza.
///
/// # Errors
/// [`Error::Allocation`] se a saída não puder ser alocada.
pub fn equalize(input: ImageView<'_, L8>) -> Result<Image<L8>> {
    let counts = histogram(input, 0, None)?;
    apply_lut(input, &equalization_lut(&counts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cumulative_of_an_empty_histogram_is_zero() {
        assert!(cumulative(&[0; 256]).iter().all(|&n| n == 0));
    }

    #[test]
    fn counts_do_not_overflow_the_scaling() {
        // Contagens enormes: o produto `n · 255 · 2` passaria de u64.
        let mut h = [0u64; 256];
        h[0] = u64::MAX / 4;
        h[255] = u64::MAX / 4;
        let lut = equalization_lut(&h);
        assert_eq!((lut.as_slice()[0], lut.as_slice()[255]), (0, 255));
    }
}
