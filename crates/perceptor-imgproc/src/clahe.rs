//! CLAHE: equalização adaptativa de histograma com limite de contraste.
//!
//! A [equalização global](crate::histogram) usa um histograma para a imagem
//! inteira e ignora que regiões diferentes pedem curvas diferentes. O CLAHE
//! (*Contrast Limited Adaptive Histogram Equalization*) a refaz por região:
//!
//! 1. A imagem é dividida em uma grade `tiles_x × tiles_y`.
//! 2. O histograma de cada tile é **cortado** em `clip_limit` vezes a altura
//!    uniforme (`área / 256`); o que passa do corte é redistribuído
//!    igualmente entre os 256 níveis. Isso limita a inclinação da curva e,
//!    com ela, a amplificação de ruído em regiões quase planas.
//! 3. Cada histograma vira uma tabela de equalização (`lut[v] =
//!    255 · cdf(v) / área`).
//! 4. Cada pixel passa pelas tabelas dos **quatro tiles** mais próximos e o
//!    resultado é a interpolação bilinear dos quatro, pela posição do pixel
//!    entre os centros dos tiles. Sem isso, a fronteira entre tiles
//!    apareceria como um degrau.
//!
//! Pixels mais perto da borda que do centro do primeiro (ou último) tile usam
//! só a tabela dele.
//!
//! # Convenções
//!
//! - `clip_limit` tem a mesma unidade do `OpenCV` (`cv2.createCLAHE`): `1.0`
//!   limita todo bin à altura uniforme e deixa a imagem quase intacta;
//!   valores a partir de `256` não cortam nada (é a AHE pura).
//! - A redistribuição do excesso também segue o `OpenCV`: o quociente inteiro
//!   vai a todos os bins e o resto, um a um, em passos regulares.
//! - Os pesos da interpolação são quantizados em Q16; o resultado é
//!   arredondado para o mais próximo.
//! - Se a imagem não for múltipla da grade, as fronteiras dos tiles caem em
//!   `i · largura / tiles_x` (divisão inteira) e os tiles diferem em até um
//!   pixel. O `OpenCV`, nesse caso, preenche a imagem até um múltiplo.

use perceptor_core::{Error, Image, ImageView, Rect, Result, L8};

use crate::histogram::{histogram, Histogram};

/// Escala dos pesos da interpolação (`1.0 = 2¹⁶`).
const WEIGHT_ONE: u64 = 1 << 16;

/// Aplica CLAHE a uma imagem em tons de cinza.
///
/// `tiles` é `(colunas, linhas)` da grade. Valores usuais: `clip_limit`
/// entre `2.0` e `4.0`, grade `(8, 8)`.
///
/// # Errors
/// - [`Error::InvalidParameter`] se `tiles` tiver um eixo zero ou maior que a
///   imagem (um tile ficaria vazio), ou se `clip_limit` não for finito e
///   positivo.
/// - [`Error::Allocation`] se a saída não puder ser alocada.
pub fn clahe(
    input: ImageView<'_, L8>,
    clip_limit: f64,
    tiles: (usize, usize),
) -> Result<Image<L8>> {
    let (width, height) = input.dimensions();
    let (tiles_x, tiles_y) = tiles;
    if tiles_x == 0 || tiles_y == 0 || tiles_x > width || tiles_y > height {
        return Err(Error::InvalidParameter {
            name: "tiles",
            reason: "cada eixo da grade precisa ficar entre 1 e a dimensão da imagem",
        });
    }
    if !clip_limit.is_finite() || clip_limit <= 0.0 {
        return Err(Error::InvalidParameter {
            name: "clip_limit",
            reason: "precisa ser finito e positivo",
        });
    }

    let columns = bounds(width, tiles_x);
    let rows = bounds(height, tiles_y);

    // Uma tabela por tile, em ordem de linha.
    let mut luts = Vec::with_capacity(tiles_x * tiles_y);
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let region = Rect::new(
                columns[tx],
                rows[ty],
                columns[tx + 1] - columns[tx],
                rows[ty + 1] - rows[ty],
            );
            let counts = histogram(input.roi(region)?, 0, None)?;
            luts.push(tile_lut(&counts, region.width * region.height, clip_limit));
        }
    }

    let across = axis_weights(&columns);
    let down = axis_weights(&rows);
    let mut output = Image::<L8>::new(width, height)?;
    for ((y, dst), src) in output.rows_mut().enumerate().zip(input.rows()) {
        let (j0, j1, wy) = down[y];
        for ((x, out), pixel) in dst.iter_mut().enumerate().zip(src) {
            let (i0, i1, wx) = across[x];
            let level = usize::from(pixel.luma());
            let at = |ty: usize, tx: usize| u64::from(luts[ty * tiles_x + tx][level]);
            let top = at(j0, i0) * (WEIGHT_ONE - wx) + at(j0, i1) * wx;
            let bottom = at(j1, i0) * (WEIGHT_ONE - wx) + at(j1, i1) * wx;
            let mixed = top * (WEIGHT_ONE - wy) + bottom * wy;
            // `mixed` está em escala 2³²; soma metade para arredondar.
            let rounded = (mixed + (WEIGHT_ONE * WEIGHT_ONE) / 2) / (WEIGHT_ONE * WEIGHT_ONE);
            *out = L8::new(u8::try_from(rounded).unwrap_or(u8::MAX));
        }
    }
    Ok(output)
}

/// Fronteiras dos `tiles` pedaços de `len` pixels: `tiles + 1` posições.
fn bounds(len: usize, tiles: usize) -> Vec<usize> {
    (0..=tiles).map(|i| i * len / tiles).collect()
}

/// Para cada posição do eixo: os dois tiles que a cercam e o peso do segundo
/// (Q16). Os centros dos tiles ficam em `(início + fim) / 2`; antes do
/// primeiro e depois do último só vale o tile da ponta.
fn axis_weights(bounds: &[usize]) -> Vec<(usize, usize, u64)> {
    let tiles = bounds.len() - 1;
    // Em unidades de meio pixel, para ficar em inteiros.
    let centers: Vec<usize> = bounds.windows(2).map(|w| w[0] + w[1]).collect();
    let len = bounds[tiles];
    (0..len)
        .map(|p| {
            let position = 2 * p; // o pixel `p` em meio pixel
            if position <= centers[0] {
                (0, 0, 0)
            } else if position >= centers[tiles - 1] {
                (tiles - 1, tiles - 1, 0)
            } else {
                let i = centers.partition_point(|&c| c <= position) - 1;
                let span = (centers[i + 1] - centers[i]) as u64;
                let offset = (position - centers[i]) as u64;
                // Arredonda para o mais próximo em Q16.
                let weight = (offset * WEIGHT_ONE * 2 + span) / (2 * span);
                (i, i + 1, weight)
            }
        })
        .collect()
}

/// Corta o histograma, redistribui o excesso e devolve a tabela do tile.
fn tile_lut(counts: &Histogram, area: usize, clip_limit: f64) -> [u8; 256] {
    // Mesma unidade do OpenCV: múltiplo da altura uniforme, no mínimo 1.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let clip = ((clip_limit * area as f64 / 256.0) as u64).max(1);

    let mut hist = *counts;
    let mut excess = 0u64;
    for bin in &mut hist {
        if *bin > clip {
            excess += *bin - clip;
            *bin = clip;
        }
    }
    let batch = excess / 256;
    let mut residual = excess - batch * 256;
    for bin in &mut hist {
        *bin += batch;
    }
    // `residual ≤ 255`, então o passo fica em `1..=256`.
    if let Some(quotient) = 256u64.checked_div(residual) {
        let step = usize::try_from(quotient.max(1)).unwrap_or(1);
        let mut i = 0;
        while i < 256 && residual > 0 {
            hist[i] += 1;
            residual -= 1;
            i += step;
        }
    }

    // `lut[v] = round(255 · cdf(v) / área)`, em inteiros.
    let area = area as u64;
    let mut sum = 0u64;
    let mut lut = [0u8; 256];
    for (entry, &bin) in lut.iter_mut().zip(&hist) {
        sum += bin;
        let scaled = (sum * 255 * 2 + area) / (2 * area);
        *entry = u8::try_from(scaled).unwrap_or(u8::MAX);
    }
    lut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_cover_the_axis_and_differ_by_at_most_one() {
        let b = bounds(10, 3);
        assert_eq!(b, [0, 3, 6, 10]);
        let widths: Vec<usize> = b.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(widths.iter().max().unwrap() - widths.iter().min().unwrap() <= 1);
    }

    #[test]
    fn axis_weights_blend_between_centers_and_clamp_at_the_ends() {
        // Dois tiles de 4 pixels: centros em 4 e 12 (meio pixel) = 2 e 6.
        let w = axis_weights(&[0, 4, 8]);
        assert_eq!(w[0], (0, 0, 0));
        assert_eq!(w[2], (0, 0, 0), "no centro do primeiro tile");
        assert_eq!(w[4], (0, 1, WEIGHT_ONE / 2), "no meio entre os centros");
        assert_eq!(w[6], (1, 1, 0), "no centro do último tile");
        assert_eq!(w[7], (1, 1, 0));
    }

    #[test]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn the_clip_limit_bounds_the_slope_of_the_tile_table() {
        // Histograma muito concentrado: 1000 pixels em dois níveis.
        let mut h = [0u64; 256];
        h[40] = 600;
        h[41] = 400;
        let area = 1000usize;
        for clip_limit in [1.0, 2.0, 4.0, 16.0] {
            let lut = tile_lut(&h, area, clip_limit);
            // Nenhum bin passa de `clip + lote + 1`; um degrau da tabela é
            // então no máximo `⌈255 · bin / área⌉ + 1` (o +1 do arredondamento).
            let clip = ((clip_limit * 1000.0 / 256.0) as u64).max(1);
            let batch = (1000 - 2 * clip) / 256 + 1;
            let max_bin = clip + batch + 1;
            let bound = (255 * max_bin).div_ceil(area as u64) + 1;
            let step = lut
                .windows(2)
                .map(|w| u64::from(w[1] - w[0]))
                .max()
                .unwrap();
            assert!(step <= bound, "clip {clip_limit}: degrau {step} > {bound}");
        }
    }

    #[test]
    fn a_clip_of_at_least_the_tile_area_does_not_clip() {
        let mut h = [0u64; 256];
        h[10] = 30;
        h[200] = 70;
        // Sem corte, a tabela é a CDF reescalada: 30% → 77, 100% → 255.
        let lut = tile_lut(&h, 100, 256.0);
        assert_eq!((lut[9], lut[10], lut[199], lut[200]), (0, 77, 77, 255));
    }
}
