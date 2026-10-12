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

## `sobel`

```rust
pub fn apply_sobel(input: ImageView<'_, L8>) -> Result<Image<L8>>
```

Magnitude do gradiente Sobel 3×3 de uma imagem em tons de cinza:

```text
G = sqrt(Gx² + Gy²), saturado em 255
```

Usa preenchimento com zeros nas bordas, portanto pixels de borda de uma imagem uniforme têm resposta não nula. Erro em runtime: só `Allocation`.
