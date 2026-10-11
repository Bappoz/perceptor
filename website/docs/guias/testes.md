---
title: Testando kernels
---

# Testando kernels

Todo kernel novo segue o mesmo roteiro, com as ferramentas de `perceptor-testkit` (crate interna, `dev-dependency` de `perceptor-imgproc`, `perceptor-pipeline` e da fachada).

```toml
[dev-dependencies]
perceptor-testkit.workspace = true
```

:::caution
`perceptor-core` não pode usar o testkit nos próprios testes. O `dev-dependency` circular compilaria duas cópias de `perceptor-core`, e o `Image<P>` de cada uma seria um tipo diferente.
:::

## Roteiro

1. **Escreva o teste que falha** antes do kernel.
2. **Propriedades** com `proptest` e [`strategies`](#estratégias): invariantes que valem para qualquer imagem (dimensões preservadas, idempotência, simetria).
3. **Casos exatos** com [`patterns`](#imagens-sintéticas): entradas pequenas em que você sabe a resposta de cabeça.
4. **Golden** para a saída completa de um caso representativo.
5. **Otimização** (SIMD, paralelismo) sempre comparada à versão escalar de referência, com `assert_images_eq!`.

## Imagens sintéticas

| Função | Resultado |
|---|---|
| `patterns::gradient::<P>(w, h)` | rampa horizontal de `0` ao fundo de escala, todos os canais iguais |
| `patterns::checkerboard::<P>(w, h, cell)` | xadrez de células `cell × cell` |
| `patterns::impulse::<P>(w, h, x, y)` | zero, com fundo de escala só em `(x, y)` — a resposta ao impulso de um filtro |
| `patterns::noise::<P>(w, h, seed)` | ruído uniforme reproduzível (xorshift64\*, sem `rand`) |

## Estratégias

`strategies::any_image::<P>(1..=32, 1..=32)` gera imagens com dimensões na faixa dada e amostras uniformes em todo o intervalo nominal do formato (`0..=255`, `0..=65535`, `0.0..=1.0`). `strategies::small_image::<P>()` é o atalho de `1..=16`. O encolhimento leva a imagens pequenas e zeradas.

```rust
proptest! {
    #[test]
    fn flip_twice_is_identity(img in strategies::small_image::<Rgb8>()) {
        assert_images_eq!(flip_h(&flip_h(&img)), img);
    }
}
```

## Comparando imagens

`assert_images_eq!(a, b)` exige igualdade exata; `assert_images_eq!(a, b, tolerance = 1.0)` aceita diferença de até 1 na unidade nativa do formato (níveis em `u8`/`u16`, fração em `f32`). Na falha:

```text
assert_images_eq! falhou: 3 de 48 amostras divergem (tolerância 0): max |Δ| = 12, PSNR = 38.41 dB
  (x=2, y=1, c=1): esquerda = 10, direita = 22
  …
```

Para métricas sem asserção: `compare::diff(a, b, tolerance)`, `compare::max_abs_diff` e `compare::psnr`.

## Golden images

`assert_golden!(imagem, "nome.png")` compara com `tests/golden/nome.png` da crate que chama; `tolerance = …` funciona como acima. Os arquivos são PNG de verdade: abra no visualizador para ver o que mudou.

Formatos suportados: `L8`, `La8`, `Rgb8`, `Rgba8` e `L16`. Para saída em ponto flutuante, quantize para `L8`/`L16` antes de comparar.

### Regenerar

```bash
PERCEPTOR_UPDATE_GOLDEN=1 cargo test -p perceptor-imgproc
```

Reescreve os arquivos em vez de compará-los. **Revise as imagens antes de commitar**: regenerar aceita qualquer saída como correta. Sem a variável, um golden ausente falha com a instrução acima.
