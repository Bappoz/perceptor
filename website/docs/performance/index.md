---
title: Performance
---

# Performance

Esta página registra as medições feitas a cada mudança de performance, com o método abaixo. O grosso do trabalho de otimização é do milestone M6.

## Método

1. Benchmark com `criterion` antes de qualquer mudança.
2. Perfil para localizar o gargalo.
3. Mudança isolada, mantendo a versão escalar como referência.
4. Teste de propriedade comparando a versão otimizada com a referência.
5. Benchmark depois, registrado no PR.

## Medições

Os números valem para a máquina em que foram tirados — Intel Core i7-1255U (12 threads), Linux, `rustc` 1.99, `criterion` 0.8 — e servem para comparar antes e depois, não como valor absoluto. Para reproduzir:

```bash
just bench -p perceptor-imgproc --bench grayscale
```

### Escala de cinza

`convert_to_gray` (BT.601) sobre ruído uniforme `Rgb8`, tempo mediano por imagem ([#47](https://github.com/Bappoz/perceptor/issues/47)):

| Imagem | `f32` truncado, uma tarefa por linha (antes) | Q15, faixas de linhas (depois) | Ganho | Referência `f64` escalar |
|---|---|---|---|---|
| 64×64 | 46,3 µs | 3,4 µs | 13,5× | 12,5 µs |
| 640×480 | 293 µs | 163 µs | 1,8× | 993 µs |
| 1920×1080 | 1,32 ms | 0,79 ms | 1,7× | 6,69 ms |

O que os números mostram:

- Em 64×64 o ganho vem quase todo de **não** paralelizar: a versão anterior despachava 64 tarefas rayon para 4 096 pixels e ficava mais lenta que a referência escalar.
- Nas imagens grandes o ganho da aritmética inteira é menor do que a diferença para a referência sugere, porque as duas versões já usavam todos os núcleos; a comparação justa é antes × depois.
- Faixas de 8 mil e de 32 mil pixels ficaram dentro do ruído uma da outra; com 128 mil, a de 640×480 caiu para ~260 µs (poucas tarefas para repartir).

## O que será medido

| Medida | Issue |
|---|---|
| throughput de grayscale e Sobel | [#14](https://github.com/Bappoz/perceptor/issues/14) |
| ganho de cada kernel SIMD | milestone M6 |
| alocações por frame no caminho de streaming | milestone M6 |
| comparação com `imageproc` e OpenCV | milestone M6 |
