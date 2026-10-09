---
title: Performance
---

# Performance

Ainda não há medições publicadas. Esta página passa a registrar resultados a partir do milestone M6, com o método abaixo.

## Método

1. Benchmark com `criterion` antes de qualquer mudança.
2. Perfil para localizar o gargalo.
3. Mudança isolada, mantendo a versão escalar como referência.
4. Teste de propriedade comparando a versão otimizada com a referência.
5. Benchmark depois, registrado no PR.

## O que será medido

| Medida | Issue |
|---|---|
| throughput de grayscale e Sobel | [#14](https://github.com/Bappoz/perceptor/issues/14) |
| ganho de cada kernel SIMD | milestone M6 |
| alocações por frame no caminho de streaming | milestone M6 |
| comparação com `imageproc` e OpenCV | milestone M6 |
