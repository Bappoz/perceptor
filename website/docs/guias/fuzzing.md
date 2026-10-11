---
title: Fuzzing
---

# Fuzzing

`perceptor-core` recebe entrada não confiável por três caminhos: dimensões e buffers de construtores, geometria de visões (stride, ROI, bordas) e conversões de/para `ndarray` e `image`. O fuzzing garante que **nenhuma entrada** vira pânico, estouro ou UB: entrada inválida tem de virar `Err`.

## Alvos

| Alvo | O que cobre |
|---|---|
| `image_ctor` | `Image::new`, `filled`, `from_subpixels`, `from_vec` com dimensões e comprimentos arbitrários, nos 8 formatos |
| `view_geometry` | `ImageView::new` (stride e overflow), `roi`, `split_at_row`, `fill`, `copy_from`, `get_bordered`, `padded_row` e `map_index` |
| `interop` | `from_array3`/`to_array3` com formas e strides arbitrários (inclusive eixos invertidos) e `from_image_buffer`/`to_image_buffer` |

Cada alvo só repassa os bytes para uma função de [`perceptor_testkit::fuzz`](https://github.com/Bappoz/perceptor/blob/master/crates/perceptor-testkit/src/fuzz.rs). A lógica fica em uma crate comum para rodar também em `cargo test`, sem nightly: o CI executa 3000 entradas pseudoaleatórias por alvo a cada PR.

## Oráculos

O harness não confere só "não pânico". Para `ImageView::new` e `roi` ele calcula, por um caminho independente (`checked_mul`/`checked_add`), se a chamada **deveria** ter sucesso e exige que o resultado `Ok`/`Err` coincida. Depois do sucesso confere as invariantes: dimensões, alinhamento de 64 bytes, conteúdo copiado idêntico, cantos da ROI alinhados com o pai e faixa inferior de `split_at_row` intacta após escrever na superior.

Comparações usam igualdade bit a bit: a entrada pode conter `NaN` em `f32`.

## Rodando

Requer `cargo-fuzz` e o toolchain nightly.

```bash
just fuzz image_ctor 60      # alvo, segundos
just fuzz view_geometry 300
```

Um crash vira um arquivo em `fuzz/artifacts/<alvo>/`. Reproduza com `cd fuzz && cargo +nightly fuzz run <alvo> artifacts/<alvo>/crash-…`, **escreva primeiro o teste de regressão** em `perceptor-core` e só então corrija.

## Corpus

`fuzz/corpus/<alvo>/` guarda apenas sementes escritas à mão (cerca de quatro por alvo). O libFuzzer reconstrói a cobertura em segundos; versionar o corpus inteiro só incharia o repositório. Para promover uma entrada que achou cobertura nova, adicione o arquivo com nome descritivo.

## Execuções registradas

Primeira rodada, 2026-10-11, 60 s por alvo, nenhum crash:

| Alvo | Execuções | Cobertura (edges) |
|---|---|---|
| `image_ctor` | 6,79 M | 907 |
| `view_geometry` | 8,84 M | 2779 |
| `interop` | 6,91 M | 1959 |
