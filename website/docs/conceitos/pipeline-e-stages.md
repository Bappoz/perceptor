---
title: Pipeline e stages
---

# Pipeline e stages

## Ciclo de um tick

`Pipeline::tick()` roda um único schedule em que cada stage é um `SystemSet`. Os seis sets são encadeados, sempre nesta ordem:

```mermaid
flowchart LR
    I[Input] --> P[PreProcess] --> C[Process] --> O[PostProcess] --> S[Output] --> L[Cleanup]
```

| Stage | Uso |
|---|---|
| `Stage::Input` | fontes criam entidades `Frame` |
| `Stage::PreProcess` | preparação (resize, normalização) |
| `Stage::Process` | filtros e detecção |
| `Stage::PostProcess` | agregação de resultados (NMS, tracking) |
| `Stage::Output` | escrita em disco, exibição |
| `Stage::Cleanup` | remoção das entidades que a saída já consumiu |

`Pipeline::run()` repete ticks até uma [condição de parada](#controle-de-parada) valer.

## Ciclo de vida dos frames

Uma fonte cria um `Frame` por tick. Sem limpeza eles se acumulam no `World` e todo sistema passa a revisitá-los. `despawn_processed_frames_system`, registrado em `Stage::Cleanup`, remove toda entidade com `Frame` depois que os sistemas de saída rodaram.

O `IoPlugin` já o registra. Para inspecionar os frames depois do tick (testes, depuração), use `keep_frames: true`. Um pipeline sem `IoPlugin` não remove nada por conta própria:

```rust
builder.add_cleanup_system(despawn_processed_frames_system);
```

Com a limpeza ligada o frame vive um único tick: tudo o que um sistema precisa ver tem de estar pronto antes de `Stage::Cleanup`, o que a [ordenação](#visibilidade-de-componentes) abaixo garante.

## Controle de parada

```rust
use perceptor::prelude::*;

let mut pipeline = Pipeline::builder().with_max_ticks(3).build()?;
let reason = pipeline.run()?; // executa exatamente 3 ticks
assert_eq!(reason, StopReason::TickLimit);
```

`run()` devolve o `StopReason` da primeira condição que valer, sempre verificada no fim do tick:

| Condição | `StopReason` |
|---|---|
| um sistema chama `PipelineState::request_stop()`, ou outra thread chama `StopHandle::stop()` | `Requested` |
| Ctrl-C, com `stop_on_ctrl_c()` (feature `ctrlc`) | `Interrupted` |
| todas as fontes registradas em `Sources` terminaram | `EndOfStream` |
| `with_max_frames(n)`, `with_max_ticks(n)`, `with_max_duration(d)` | `FrameLimit`, `TickLimit`, `TimeLimit` |

Uma fonte finita, como o `IoPlugin`, encerra o `run()` sozinha. Detalhes e exemplo em [Condições de parada](../guias/condicoes-de-parada.mdx).

## Visibilidade de componentes

`Commands` (inserir componente, criar ou remover entidade) são adiados até um **ponto de sincronização**. Há um entre cada par de stages consecutivos e um entre dois sistemas ordenados explicitamente.

| Situação | O segundo sistema vê os `Commands` do primeiro? |
|---|---|
| stages diferentes, mesmo tick | sim |
| mesmo stage, ordenados com `.chain()` / `.before()` / `.after()` | sim |
| mesmo stage, sem ordem | não — só no tick seguinte, e a ordem de execução é indefinida |

```rust
// O Sobel exige o `GrayscaleTag` que o grayscale insere via `Commands`.
builder.add_system(Stage::Process, (grayscale_system, sobel_system).chain());
```

É o que `FiltersPlugin::all()` faz: grayscale e Sobel rodam no mesmo tick.

Os sistemas rodam um de cada vez, na thread que chamou `tick()`: o pipeline fixa o executor single-thread do `bevy_ecs`, e o paralelismo fica dentro dos kernels (rayon). Entre sistemas sem ordem a sequência é indefinida — ordene o que depende de fato.

## Tratamento de erros

Kernels e funções de I/O retornam [`perceptor_core::Result`](../referencia/perceptor-core.mdx). Um sistema ECS não tem para quem devolver esse `Result`, então o entrega ao recurso `PipelineErrors`:

- **falha do sistema inteiro** (a fonte não abre, a configuração é inválida): o sistema retorna `Result<()>` e é registrado com `fallible(sistema)`;
- **falha de um frame**: o sistema chama `errors.report_frame("nome", entidade, erro)` e segue para os próximos frames.

O que acontece depois depende da `ErrorPolicy`, definida com `PipelineBuilder::with_error_policy`:

| Política | Stages seguintes no tick | Frame com erro | `tick()` |
|---|---|---|---|
| `FailFast` (padrão) | não rodam | permanece no `World` | `Err` com o primeiro erro |
| `SkipFrame` | rodam | removido ao fim do stage em que falhou | `Ok` |
| `Continue` | rodam | segue pelos stages | `Ok` |

`run()` propaga o `Err` de `tick()`, portanto só falha em `FailFast`.

`pipeline.errors()` lista os erros do tick mais recente (sistema, frame e erro) e `total()` conta todos desde a criação do pipeline. A lista é esvaziada no início de cada tick. Em `FailFast`, o erro devolvido por `tick()` sai da lista; os demais do mesmo tick ficam nela.

Detalhes que valem conhecer:

- Em `FailFast` a interrupção é **por stage**: os outros sistemas do stage em que o erro ocorreu ainda rodam, e `Stage::Cleanup` não roda — os frames ficam no `World` para inspeção.
- Em `SkipFrame`, um erro sem frame associado (`fallible`, `report`) só fica registrado.
- Um `World` montado à mão para rodar `grayscale_system`, `sobel_system` ou `image_writer_system` precisa do recurso: `world.init_resource::<PipelineErrors>()`.

Exemplo completo em [Sistemas que podem falhar](../guias/escrevendo-um-plugin.mdx#sistemas-que-podem-falhar).
