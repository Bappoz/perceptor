---
title: perceptor-imgproc
---

# perceptor-imgproc

Kernels de processamento de imagem como funções puras.

## `grayscale`

```rust
pub fn convert_to_gray(input: &Array3<u8>) -> Result<Array3<u8>>
```

Converte RGB `[H, W, 3]` em luminância `[H, W, 1]` com os pesos BT.601:

```text
Y = 0.299·R + 0.587·G + 0.114·B
```

O resultado é truncado (não arredondado).

| Erro | Condição |
|---|---|
| `ChannelMismatch { expected: 3, .. }` | entrada sem 3 canais |
| `NonContiguous` | tensor fora de ordem de linha |

## `sobel`

```rust
pub fn apply_sobel(input: &Array3<u8>) -> Result<Array3<u8>>
```

Magnitude do gradiente Sobel 3×3 de uma imagem em tons de cinza `[H, W, 1]`:

```text
G = sqrt(Gx² + Gy²), saturado em 255
```

Usa preenchimento com zeros nas bordas, portanto pixels de borda de uma imagem uniforme têm resposta não nula.

| Erro | Condição |
|---|---|
| `ChannelMismatch { expected: 1, .. }` | entrada sem 1 canal |
| `NonContiguous` | tensor fora de ordem de linha |
