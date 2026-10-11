---
title: Testando kernels e sistemas
---

# Testando kernels e sistemas

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

## Testando sistemas e plugins

Kernels são testados direto, como funções. Para o que depende do ECS — um sistema, ou um plugin inteiro — use o harness `perceptor_pipeline::testing::TestPipeline`: ele injeta frames a partir de imagens, roda ticks e verifica os componentes de cada entidade, sem I/O.

```rust
use perceptor_pipeline::testing::TestPipeline;

#[test]
fn converts_rgb_frames_and_tags_them() {
    let mut test = TestPipeline::with_systems(Stage::Process, grayscale_system);
    let frame = test.spawn(Image::filled(2, 2, Rgb8::new(255, 0, 0)).unwrap());

    test.tick().unwrap();

    test.assert_has::<GrayscaleTag>(frame);
    assert_eq!(test.image::<L8>(frame).as_pixels(), [L8::new(76); 4]);
}
```

O mesmo teste sem o harness precisa montar `World`, `Schedule` e recursos à mão e consultar o resultado com uma query.

### Construção

| Construtor | Uso |
|---|---|
| `TestPipeline::new()` | pipeline vazio |
| `TestPipeline::with_systems(stage, sistemas)` | um sistema, uma tupla ou `(a, b).chain()` |
| `TestPipeline::with_plugin(plugin)` | um plugin sem dependências |
| `TestPipeline::from_builder(builder)` | política de erro, vários plugins, recursos próprios |

### Frames

| Método | Efeito |
|---|---|
| `spawn(imagem)` | cria um frame com fonte `"test"` e índice sequencial; devolve a `Entity` |
| `spawn_frame(frame)` | idem, com metadados escolhidos pelo teste |
| `frames()` | entidades com `Frame` ainda vivas, por índice |
| `frame(e)` / `image::<P>(e)` | o frame, ou a imagem já no formato `P` |
| `component::<C>(e)` | `Option<&C>` |

Imagens de entrada vêm de [`patterns`](#imagens-sintéticas) ou dos construtores de `Image`.

### Verificações

| Método | Falha quando |
|---|---|
| `assert_has::<C>(e)` | `e` não tem `C` |
| `assert_lacks::<C>(e)` | `e` tem `C` |
| `assert_has_all::<(A, B, C)>(e)` | falta algum; a mensagem lista os ausentes |
| `assert_lacks_all::<(A, B)>(e)` | tem algum; a mensagem lista os presentes |
| `assert_alive(e)` / `assert_despawned(e)` | a entidade não está no estado esperado |

As falhas apontam para a linha do teste. `image::<P>` com formato errado falha mostrando o `FormatMismatch`.

`tick()` e `run_ticks(n)` devolvem o `Result` do pipeline, então a [política de erro](../conceitos/pipeline-e-stages.md#tratamento-de-erros) do builder vale no teste; `errors()` expõe o que os sistemas reportaram. Para o que o harness não cobre, `world_mut()` e `pipeline()` dão acesso direto.

Para um componente inserido por um sistema que não está no teste (ex.: o `GrayscaleTag` que o Sobel exige), insira-o à mão: `test.world_mut().entity_mut(frame).insert(GrayscaleTag)`.
