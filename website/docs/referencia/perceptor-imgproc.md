---
title: perceptor-imgproc
---

# perceptor-imgproc

Kernels de processamento de imagem como funções puras.

## `grayscale`

```rust
pub fn convert_to_gray(input: ImageView<'_, Rgb8>) -> Result<Image<L8>>
pub fn convert_to_gray_with(input: ImageView<'_, Rgb8>, weights: LumaWeights) -> Result<Image<L8>>
```

Converte RGB em luminância, `Y = wr·R + wg·G + wb·B`, com os pesos de `LumaWeights`:

| `LumaWeights` | `wr` | `wg` | `wb` | Uso |
|---|---|---|---|---|
| `Bt601` (padrão) | 0.299 | 0.587 | 0.114 | SDTV, JPEG; o `RGB → cinza` do OpenCV |
| `Bt709` | 0.2126 | 0.7152 | 0.0722 | HDTV, sRGB |

`convert_to_gray` usa `Bt601`. Os pesos são aplicados diretamente aos valores de 8 bits (luma, sem linearizar a gama), como nas outras bibliotecas.

### Ponto fixo

A conta é inteira, em Q15. Cada peso vira `round(w · 2¹⁵)`, ajustado para os três somarem exatamente `2¹⁵`:

```text
Y = (Wr·R + Wg·G + Wb·B + 2¹⁴) >> 15

Bt601: [9798, 19235, 3735]      Bt709: [6966, 23436, 2366]
```

Consequências:

- **Arredondado, não truncado**: o `+ 2¹⁴` leva ao nível mais próximo. Verde puro em BT.601 dá `150` (`149.685`), não `149`.
- **Cinza é preservado**: como os pesos somam `2¹⁵`, um pixel `(v, v, v)` resulta exatamente em `v`.
- **No máximo um nível de erro** em relação ao cálculo em `f64`. Um teste percorre os 2²⁴ valores RGB: 22 411 diferem em um nível com `Bt601` e 18 681 com `Bt709` (cerca de 0,1%); nenhum difere em mais.
- Quinze bits é o máximo em que os pesos ainda cabem em `i16`.

`grayscale::reference::convert_to_gray(input, weights)` é a versão escalar em `f64`, mantida como oráculo dos testes e base dos benchmarks.

### Paralelismo

Abaixo de `PARALLEL_THRESHOLD` (65 536 pixels) a conversão roda na thread que chamou: para imagens pequenas, acordar o pool do rayon custa mais que converter. Acima disso a saída é dividida em faixas de linhas de cerca de 32 mil pixels, uma tarefa por faixa. Medições em [Performance](../performance/index.md#escala-de-cinza).

A entrada é uma visão: aceita uma imagem inteira (`image.view()`) ou um recorte com stride maior que a linha.

O formato faz parte do tipo, então entrada com formato errado é erro de compilação. O único erro em runtime é `Allocation`, se a saída não puder ser alocada.

## `channels`

Conversões de layout de canais; nenhuma faz conta com os valores. Guia: [Cor e canais](../guias/cor-e-canais.mdx).

```rust
pub fn rgb_to_bgr(input: ImageView<'_, Rgb8>) -> Result<Image<Bgr8>>
pub fn bgr_to_rgb(input: ImageView<'_, Bgr8>) -> Result<Image<Rgb8>>
pub fn add_alpha<P: Opaque>(input: ImageView<'_, P>, alpha: u8) -> Result<Image<P::WithAlpha>>
pub fn drop_alpha<P: Alpha>(input: ImageView<'_, P>) -> Result<Image<P::Opaque>>
pub fn bgr_to_rgba(input: ImageView<'_, Bgr8>, alpha: u8) -> Result<Image<Rgba8>>
pub fn rgba_to_bgr(input: ImageView<'_, Rgba8>) -> Result<Image<Bgr8>>
pub fn swap_channels<P: Pixel>(image: ImageViewMut<'_, P>, a: usize, b: usize) -> Result<()>
pub fn extract_channel<P: Planar>(input: ImageView<'_, P>, index: usize) -> Result<Image<P::Plane>>
pub fn split_channels<P: Planar>(input: ImageView<'_, P>) -> Result<Vec<Image<P::Plane>>>
pub fn merge_channels<P: Planar>(planes: &[ImageView<'_, P::Plane>]) -> Result<Image<P>>
```

| Trait | Implementada por | Tipo associado |
|---|---|---|
| `Opaque` | `Rgb8`, `L8` | `WithAlpha`: `Rgba8`, `La8` |
| `Alpha` | `Rgba8`, `La8` | `Opaque`: `Rgb8`, `L8` |
| `Planar` | `La8`, `Rgb8`, `Bgr8`, `Rgba8`, `RgbF32` | `Plane`: `L8` (`LF32` para `RgbF32`) |

Índices de canal seguem a ordem em memória. Só `swap_channels` opera no lugar.

## `hsv`

Guia: [HSV e HSL](../guias/cor-e-canais.mdx#hsv-e-hsl).

```rust
pub fn rgb_to_hsv<P: RgbColor>(input: ImageView<'_, P>) -> Result<Image<P::Hsv>>
pub fn hsv_to_rgb<P: HsvColor>(input: ImageView<'_, P>) -> Result<Image<P::Rgb>>
pub fn rgb_to_hsl<P: RgbColor>(input: ImageView<'_, P>) -> Result<Image<P::Hsl>>
pub fn hsl_to_rgb<P: HslColor>(input: ImageView<'_, P>) -> Result<Image<P::Rgb>>
```

| Trait | Implementada por | Tipos associados |
|---|---|---|
| `RgbColor` | `Rgb8`, `RgbF32` | `Hsv`: `Hsv8`, `HsvF32`; `Hsl`: `Hsl8`, `HslF32` |
| `HsvColor` | `Hsv8`, `HsvF32` | `Rgb` |
| `HslColor` | `Hsl8`, `HslF32` | `Rgb` |

Matiz em `0..180` (graus / 2) nos formatos de 8 bits e em `0.0..360.0` nos de `f32`. Constantes `HSV8_ROUND_TRIP_TOLERANCE` (4), `HSL8_ROUND_TRIP_TOLERANCE` (5) e `F32_ROUND_TRIP_TOLERANCE` (`1e-5`): erro máximo por canal na ida e volta a partir de RGB. Erro em runtime: só `Allocation`.

## `threshold`

Guia: [Limiarização](../guias/limiarizacao.mdx).

```rust
pub fn threshold(input: ImageView<'_, L8>, thresh: u8, max: u8, kind: ThresholdType) -> Result<Image<L8>>
pub fn threshold_in_place(image: ImageViewMut<'_, L8>, thresh: u8, max: u8, kind: ThresholdType)
pub fn otsu_level(input: ImageView<'_, L8>) -> u8
pub fn otsu_from_histogram(histogram: &[u64; 256]) -> u8
pub fn threshold_otsu(input: ImageView<'_, L8>, max: u8, kind: ThresholdType) -> Result<(u8, Image<L8>)>
```

`ThresholdType`: `Binary` (padrão), `BinaryInv`, `Trunc`, `ToZero`, `ToZeroInv`; `ThresholdType::apply(v, t, max)` aplica o tipo a um nível. A comparação é `v > t`. Erro em runtime: só `Allocation`.

## `arith`

Guia: [Aritmética e lógica](../guias/aritmetica.mdx).

```rust
pub fn add<P: Pixel>(a: ImageView<'_, P>, b: ImageView<'_, P>, mask: Option<ImageView<'_, L8>>) -> Result<Image<P>>
pub fn sub<P: Pixel>(a: ImageView<'_, P>, b: ImageView<'_, P>, mask: Option<ImageView<'_, L8>>) -> Result<Image<P>>
pub fn absdiff<P: Pixel>(a: ImageView<'_, P>, b: ImageView<'_, P>, mask: Option<ImageView<'_, L8>>) -> Result<Image<P>>
pub fn add_weighted<P: Pixel>(a: ImageView<'_, P>, alpha: f64, b: ImageView<'_, P>, beta: f64, gamma: f64, mask: Option<ImageView<'_, L8>>) -> Result<Image<P>>
pub fn bitwise_and<P: Pixel>(a: ImageView<'_, P>, b: ImageView<'_, P>, mask: Option<ImageView<'_, L8>>) -> Result<Image<P>>
pub fn bitwise_or<P: Pixel>(/* idem */) -> Result<Image<P>>
pub fn bitwise_xor<P: Pixel>(/* idem */) -> Result<Image<P>>
pub fn bitwise_not<P: Pixel>(a: ImageView<'_, P>, mask: Option<ImageView<'_, L8>>) -> Result<Image<P>>
```

`P::Subpixel` é `u8`, `u16` ou `f32` (trait `Arith`); as funções `bitwise_*` exigem `u8` ou `u16` (trait `Bits`). Máscara: liga onde `≠ 0`; fora dela, a saída é o primeiro operando. Erros: `ShapeMismatch` (operando ou máscara de outras dimensões) e `Allocation`.

## `convert`

Guia: [Profundidade e normalização](../guias/profundidade-e-normalizacao.mdx).

```rust
pub fn convert_scale<P: Pixel, Q: Pixel>(input: ImageView<'_, P>, alpha: f64, beta: f64) -> Result<Image<Q>>
pub fn convert_depth<P: Pixel, Q: Pixel>(input: ImageView<'_, P>) -> Result<Image<Q>>
pub fn normalize_minmax<P: Pixel, Q: Pixel<Subpixel = f32>>(input: ImageView<'_, P>, lo: f32, hi: f32) -> Result<Image<Q>>
pub fn channel_mean_std<P: Pixel>(input: ImageView<'_, P>) -> Vec<ChannelStats>
pub fn normalize_mean_std<P: Pixel, Q: Pixel<Subpixel = f32>>(input: ImageView<'_, P>, mean: &[f32], std: &[f32]) -> Result<Image<Q>>
pub struct ChannelStats { pub mean: f64, pub std: f64 }
```

`P::Subpixel` e `Q::Subpixel` são `u8`, `u16` ou `f32` (trait `Sample`). Origem e destino têm o mesmo número de canais. Erros: `ChannelMismatch` (canais de origem/destino diferentes; `mean`/`std` com tamanho errado), `InvalidParameter` (`std` não positivo ou não finito, `mean` ou limites não finitos) e `Allocation`.

## `clahe`

Guia: [CLAHE](../guias/histograma.mdx#clahe).

```rust
pub fn clahe(input: ImageView<'_, L8>, clip_limit: f64, tiles: (usize, usize)) -> Result<Image<L8>>
```

`tiles` é `(colunas, linhas)`. Erros: `InvalidParameter` (`tiles` com um eixo zero ou maior que a imagem; `clip_limit` não finito ou `≤ 0`) e `Allocation`.

## `histogram`

Guia: [Histograma e equalização](../guias/histograma.mdx).

```rust
pub type Histogram = [u64; 256];
pub fn histogram<P: Pixel<Subpixel = u8>>(input: ImageView<'_, P>, channel: usize, mask: Option<ImageView<'_, L8>>) -> Result<Histogram>
pub fn histograms<P: Pixel<Subpixel = u8>>(input: ImageView<'_, P>, mask: Option<ImageView<'_, L8>>) -> Result<Vec<Histogram>>
pub fn cumulative(histogram: &Histogram) -> Histogram
pub fn equalization_lut(histogram: &Histogram) -> Lut<u8>
pub fn equalize(input: ImageView<'_, L8>) -> Result<Image<L8>>
```

A máscara conta onde é diferente de `0`. Erros: `ChannelOutOfRange` (canal fora do formato), `ShapeMismatch` (máscara de outro tamanho) e `Allocation` (`equalize`).

## `lut`

Guia: [Tabelas de consulta](../guias/lut-e-gamma.mdx).

```rust
pub struct Lut<T: LutSample>   // T = u8 (256 entradas) ou u16 (65 536)
impl<T: LutSample> Lut<T> {
    pub fn identity() -> Self
    pub fn invert() -> Self
    pub fn gamma(gamma: f64) -> Self
    pub fn gain_offset(gain: f64, offset: f64) -> Self
    pub fn from_fn(f: impl FnMut(T) -> T) -> Self
    pub fn as_slice(&self) -> &[T]
}
pub fn apply_lut<P: Pixel>(input: ImageView<'_, P>, lut: &Lut<P::Subpixel>) -> Result<Image<P>>
pub fn apply_lut_in_place<P: Pixel>(image: ImageViewMut<'_, P>, lut: &Lut<P::Subpixel>)
pub fn apply_lut_per_channel<P: Pixel>(input: ImageView<'_, P>, luts: &[Lut<P::Subpixel>]) -> Result<Image<P>>
```

`P::Subpixel` precisa ser `u8` ou `u16`; formatos `f32` não compilam. `apply_lut` aplica a mesma tabela a todos os canais, alfa incluído. Erros: `ChannelMismatch` (`apply_lut_per_channel` com número errado de tabelas) e `Allocation`.

## `adaptive`

Guia: [Limiar adaptativo](../guias/limiarizacao.mdx#limiar-adaptativo).

```rust
pub fn adaptive_threshold(input: ImageView<'_, L8>, max: u8, method: AdaptiveMethod, block_size: usize, c: i32) -> Result<Image<L8>>
pub fn adaptive_threshold_inv(input: ImageView<'_, L8>, max: u8, method: AdaptiveMethod, block_size: usize, c: i32) -> Result<Image<L8>>
```

`AdaptiveMethod`: `Mean` (padrão) ou `Gaussian`. Liga o pixel quando `v + c > média_local`; a versão `_inv` devolve o complemento. Borda replicada; janela maior que a imagem é aceita. Erros: `InvalidWindowSize` (`block_size` par, menor que 3 ou grande demais para somar em `u64`) e `Allocation`.

## `yuv`

Guia: [YUV → RGB](../guias/cor-e-canais.mdx#yuv--rgb).

```rust
pub fn i420_to_rgb(y: ImageView<'_, L8>, u: ImageView<'_, L8>, v: ImageView<'_, L8>, spec: YuvSpec) -> Result<Image<Rgb8>>
pub fn nv12_to_rgb(y: ImageView<'_, L8>, uv: ImageView<'_, L8>, spec: YuvSpec) -> Result<Image<Rgb8>>
pub fn yuyv_to_rgb(packed: ImageView<'_, L8>, spec: YuvSpec) -> Result<Image<Rgb8>>
```

`YuvSpec { matrix: YuvMatrix, range: YuvRange }`, com `BT601_LIMITED` (padrão), `BT601_FULL`, `BT709_LIMITED` e `BT709_FULL`. Ponto fixo Q14, a no máximo um nível de `yuv::reference::yuv_to_rgb(y, cb, cr, spec)`. Croma sem interpolação. Erros: `OddDimensions`, `ShapeMismatch`, `BufferLength`, `Allocation`.

## `sobel`

```rust
pub fn apply_sobel(input: ImageView<'_, L8>) -> Result<Image<L8>>
```

Magnitude do gradiente Sobel 3×3 de uma imagem em tons de cinza:

```text
G = sqrt(Gx² + Gy²), saturado em 255
```

Usa preenchimento com zeros nas bordas, portanto pixels de borda de uma imagem uniforme têm resposta não nula. Erro em runtime: só `Allocation`.
