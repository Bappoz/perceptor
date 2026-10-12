//! Limiarização adaptativa: o limiar de cada pixel vem da vizinhança dele.
//!
//! Um limiar global falha quando a iluminação varia pela imagem (uma página
//! fotografada com sombra, por exemplo). Aqui cada pixel é comparado com a
//! **média local** da janela `block_size × block_size` centrada nele, menos
//! uma constante `c`:
//!
//! ```text
//! ligado  ⇔  v > média_local − c   ⇔   v + c > média_local
//! ```
//!
//! `c > 0` torna o limiar mais permissivo (mais pixels ligados); `c < 0`, mais
//! rígido. A comparação é **exata**: as médias são somas inteiras e a divisão
//! pela área nunca acontece, então não há erro de arredondamento no limite.
//!
//! | Método | Peso dentro da janela |
//! |---|---|
//! | [`Mean`](AdaptiveMethod::Mean) | igual (caixa), por somas deslizantes em O(1) por pixel |
//! | [`Gaussian`](AdaptiveMethod::Gaussian) | gaussiano separável em ponto fixo Q16 |
//!
//! # Bordas
//!
//! A borda é **replicada**: fora da imagem vale o pixel mais próximo. Uma
//! janela maior que a imagem é aceita.
//!
//! # Gaussiana
//!
//! `σ = 0,3·((block_size − 1)/2 − 1) + 0,8`, a mesma regra do `OpenCV`. Os
//! pesos são arredondados para inteiros que somam exatamente `2¹⁶` (o resto
//! vai para o centro, preservando a simetria), o que dá duas propriedades
//! testáveis: uma imagem constante tem média exatamente igual ao próprio
//! valor, e uma rampa linear também, longe da borda.

use perceptor_core::{Error, Image, ImageView, Result, L8};

/// Como a média local é ponderada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AdaptiveMethod {
    /// Todos os pixels da janela pesam igual. É o padrão.
    #[default]
    Mean,
    /// Pesos gaussianos: o centro pesa mais que a borda da janela.
    Gaussian,
}

/// Soma dos pesos do kernel gaussiano em ponto fixo (`2¹⁶`).
const GAUSS_ONE: u32 = 1 << 16;

/// Limiariza com a média local: `max` onde `v + c > média`, `0` nos demais.
///
/// `block_size` é o lado da janela: ímpar e maior que 1.
///
/// # Errors
/// - [`Error::InvalidWindowSize`] se `block_size` for par, menor que 3 ou
///   grande demais para somar sem estouro.
/// - [`Error::Allocation`] se a saída não puder ser alocada.
pub fn adaptive_threshold(
    input: ImageView<'_, L8>,
    max: u8,
    method: AdaptiveMethod,
    block_size: usize,
    c: i32,
) -> Result<Image<L8>> {
    run(input, (max, 0), method, block_size, c)
}

/// Como [`adaptive_threshold`], com a saída invertida: `0` onde `v + c >
/// média`, `max` nos demais.
///
/// # Errors
/// Os mesmos de [`adaptive_threshold`].
pub fn adaptive_threshold_inv(
    input: ImageView<'_, L8>,
    max: u8,
    method: AdaptiveMethod,
    block_size: usize,
    c: i32,
) -> Result<Image<L8>> {
    run(input, (0, max), method, block_size, c)
}

/// `on`/`off` são os valores de saída para "acima" e "não acima" da média.
fn run(
    input: ImageView<'_, L8>,
    (on, off): (u8, u8),
    method: AdaptiveMethod,
    block_size: usize,
    c: i32,
) -> Result<Image<L8>> {
    // 255·block² precisa caber em u64 para as somas não estourarem.
    let valid = block_size >= 3
        && !block_size.is_multiple_of(2)
        && block_size
            .checked_mul(block_size)
            .and_then(|area| area.checked_mul(255))
            .is_some();
    if !valid {
        return Err(Error::InvalidWindowSize { size: block_size });
    }
    let radius = block_size / 2;
    let (width, height) = (input.width(), input.height());
    let mut output = Image::<L8>::new(width, height)?;

    // `sums` é a média local multiplicada por `scale`: comparar
    // `(v + c)·scale > sums` equivale a `v + c > média`, sem divisão.
    let (sums, scale) = match method {
        AdaptiveMethod::Mean => (mean_sums(input, radius), (block_size * block_size) as u128),
        AdaptiveMethod::Gaussian => (
            gaussian_sums(input, &gaussian_kernel(block_size)),
            u128::from(GAUSS_ONE) * u128::from(GAUSS_ONE),
        ),
    };
    let scale = i128::try_from(scale).expect("área limitada por `valid`");
    let c = i128::from(c);

    let row_sums = sums.chunks_exact(width);
    for ((dst, src), sums) in output.rows_mut().zip(input.rows()).zip(row_sums) {
        for ((out, pixel), &sum) in dst.iter_mut().zip(src).zip(sums) {
            let above = (i128::from(pixel.luma()) + c) * scale > i128::from(sum);
            *out = L8::new(if above { on } else { off });
        }
    }
    Ok(output)
}

/// Janela `[i − r, i + r]` sobre `n` amostras com borda replicada: índices
/// `lo..=hi` dentro do vetor e quantas amostras caem antes/depois dele.
const fn span(i: usize, r: usize, n: usize) -> (usize, usize, u64, u64) {
    let lo = i.saturating_sub(r);
    let hi = if i + r < n { i + r } else { n - 1 };
    let before = r.saturating_sub(i) as u64;
    let after = (i + r).saturating_sub(n - 1) as u64;
    (lo, hi, before, after)
}

/// Soma de cada janela `(2r+1)²`, com borda replicada. Duas passadas de
/// somas por prefixo: O(1) por pixel, qualquer que seja `r`.
fn mean_sums(input: ImageView<'_, L8>, r: usize) -> Vec<u64> {
    let (w, h) = (input.width(), input.height());

    // Passada horizontal: `rows[y·w + x]` soma a janela na linha `y`.
    let mut rows = vec![0u64; w * h];
    let mut prefix = vec![0u64; w + 1];
    for (src, dst) in input.rows().zip(rows.chunks_exact_mut(w)) {
        for (i, pixel) in src.iter().enumerate() {
            prefix[i + 1] = prefix[i] + u64::from(pixel.luma());
        }
        let (first, last) = (u64::from(src[0].luma()), u64::from(src[w - 1].luma()));
        for (x, out) in dst.iter_mut().enumerate() {
            let (lo, hi, before, after) = span(x, r, w);
            *out = before * first + (prefix[hi + 1] - prefix[lo]) + after * last;
        }
    }

    // Passada vertical sobre as somas horizontais, também por prefixo.
    let mut cumulative = vec![0u64; (h + 1) * w];
    for y in 0..h {
        for x in 0..w {
            cumulative[(y + 1) * w + x] = cumulative[y * w + x] + rows[y * w + x];
        }
    }
    let mut out = vec![0u64; w * h];
    for (y, dst) in out.chunks_exact_mut(w).enumerate() {
        let (lo, hi, before, after) = span(y, r, h);
        let (first, last) = (&rows[..w], &rows[(h - 1) * w..]);
        let (top, bottom) = (&cumulative[lo * w..][..w], &cumulative[(hi + 1) * w..][..w]);
        for x in 0..w {
            dst[x] = before * first[x] + (bottom[x] - top[x]) + after * last[x];
        }
    }
    out
}

/// Kernel gaussiano 1D de `block` pesos inteiros que somam [`GAUSS_ONE`].
fn gaussian_kernel(block: usize) -> Vec<u32> {
    #![allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap
    )]
    let radius = (block / 2) as i64;
    let sigma = 0.3 * ((block as f64 - 1.0) * 0.5 - 1.0) + 0.8;
    let raw: Vec<f64> = (-radius..=radius)
        .map(|i| (-((i * i) as f64) / (2.0 * sigma * sigma)).exp())
        .collect();
    let total: f64 = raw.iter().sum();
    let mut weights: Vec<i64> = raw
        .iter()
        .map(|g| (g / total * f64::from(GAUSS_ONE)).round() as i64)
        .collect();
    // O resto do arredondamento vai para o centro: mantém a simetria e a soma exata.
    let remainder = i64::from(GAUSS_ONE) - weights.iter().sum::<i64>();
    weights[radius as usize] += remainder;
    weights.into_iter().map(|w| w as u32).collect()
}

/// Soma ponderada de cada janela com o kernel separável `kernel` (Q16 por
/// eixo, então o resultado vale `média · 2³²`), borda replicada.
fn gaussian_sums(input: ImageView<'_, L8>, kernel: &[u32]) -> Vec<u64> {
    let (w, h) = (input.width(), input.height());
    let r = kernel.len() / 2;

    // Horizontal: cada linha é estendida com a borda replicada e varrida pelo kernel.
    let mut rows = vec![0u64; w * h];
    let mut padded = vec![0u8; w + 2 * r];
    for (src, dst) in input.rows().zip(rows.chunks_exact_mut(w)) {
        padded[..r].fill(src[0].luma());
        padded[r + w..].fill(src[w - 1].luma());
        for (slot, pixel) in padded[r..r + w].iter_mut().zip(src) {
            *slot = pixel.luma();
        }
        for (out, window) in dst.iter_mut().zip(padded.windows(kernel.len())) {
            *out = window
                .iter()
                .zip(kernel)
                .map(|(&v, &k)| u64::from(v) * u64::from(k))
                .sum();
        }
    }

    // Vertical: cada linha de saída é a soma ponderada das linhas vizinhas.
    let mut out = vec![0u64; w * h];
    for (y, dst) in out.chunks_exact_mut(w).enumerate() {
        for (j, &k) in kernel.iter().enumerate() {
            let source = (y + j).saturating_sub(r).min(h - 1);
            let line = &rows[source * w..][..w];
            for (acc, &v) in dst.iter_mut().zip(line) {
                *acc += v * u64::from(k);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaussian_kernel_is_symmetric_and_sums_to_one() {
        for block in [3, 5, 7, 11, 25, 101] {
            let kernel = gaussian_kernel(block);
            assert_eq!(kernel.len(), block);
            assert_eq!(kernel.iter().sum::<u32>(), GAUSS_ONE, "{block}");
            let reversed: Vec<u32> = kernel.iter().rev().copied().collect();
            assert_eq!(kernel, reversed, "{block}");
            assert!(kernel[block / 2] >= kernel[0]);
        }
    }

    #[test]
    fn span_counts_the_replicated_samples() {
        // n = 5, r = 2: em x = 0 duas amostras caem antes; em x = 4, duas depois.
        assert_eq!(span(0, 2, 5), (0, 2, 2, 0));
        assert_eq!(span(2, 2, 5), (0, 4, 0, 0));
        assert_eq!(span(4, 2, 5), (2, 4, 0, 2));
        // Janela maior que a linha: n = 2, r = 3.
        assert_eq!(span(0, 3, 2), (0, 1, 3, 2));
    }
}
