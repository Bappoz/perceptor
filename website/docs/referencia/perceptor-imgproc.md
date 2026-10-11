---
title: perceptor-imgproc
---

# perceptor-imgproc

Kernels de processamento de imagem como funções puras.

## `grayscale`

```rust
pub fn convert_to_gray(input: ImageView<'_, Rgb8>) -> Result<Image<L8>>
```

Converte RGB em luminância com os pesos BT.601:

```text
Y = 0.299·R + 0.587·G + 0.114·B
```

O resultado é truncado (não arredondado). A entrada é uma visão: aceita uma imagem inteira (`image.view()`) ou um recorte com stride maior que a linha.

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
