---
title: perceptor-pipeline
---

# perceptor-pipeline

Orquestração ECS sobre `bevy_ecs`.

## Tipos principais

| Item | Descrição |
|---|---|
| `Pipeline` | dono do `World` e do schedule; `tick()`, `run() -> Result<StopReason>`, `stop_reason()`, `stop_handle()`, `shutdown()`, `errors()`, `metrics()`, `world()`, `world_mut()` |
| `PipelineBuilder` | `add_plugin`, `add_*_system`, `add_op`, `with_max_ticks`, `with_max_frames`, `with_max_duration`, `stop_on_ctrl_c` (feature `ctrlc`), `with_error_policy`, `build() -> Result<Pipeline>` |
| `PipelineState` | recurso com `tick_count` e `frame_count`; `request_stop()`, `should_stop()`, `stop_reason()` |
| `StopReason` | `Requested`, `Interrupted`, `EndOfStream`, `FrameLimit`, `TickLimit`, `TimeLimit` — ver [Condições de parada](../guias/condicoes-de-parada.mdx) |
| `Sources` | recurso em que fontes finitas avisam o fim: `register`, `finish`, `is_finished`, `all_finished` |
| `StopHandle` | pedido de parada de outra thread: `stop()`, `is_stopped()` |
| `ErrorPolicy` | `FailFast` (padrão), `SkipFrame`, `Continue` |
| `PipelineErrors` | recurso com os erros do tick: `report`, `report_frame`, `iter`, `len`, `total`, `policy` |
| `SystemError` | `system`, `frame: Option<Entity>`, `error` |
| `fallible(sistema)` | adapta um sistema que retorna `Result<()>` |
| `ops::Op` | kernel puro usável como sistema: `input_format()`, `apply(&mut Frame)`, `tagged::<T>()` — ver [Kernels como sistemas](../guias/kernels-como-sistemas.mdx) |
| `PipelineMetrics` | recurso (feature `metrics`): `stage(Stage)`, `tick()`, `frames_processed()`, `frames_dropped()`, `fps()` — ver [Métricas e tracing](../guias/metricas.mdx) |
| `Timing` | `last`, `max`, `total`, `runs`, `mean()` |
| `testing::TestPipeline` | harness de teste: `with_systems`, `with_plugin`, `spawn`, `tick`, `assert_has*`, `assert_lacks*` — ver [Testando sistemas e plugins](../guias/testes.md#testando-sistemas-e-plugins) |
| `Plugin` | trait de extensão: `name`, `dependencies`, `build`, `finish`, `cleanup` |
| `Frame` | componente com `meta: FrameMeta` e `data: FrameData`; `as_image::<P>()`, `as_image_mut::<P>()`, `format()`, `width()`, `height()`, `channels()` |
| `FrameData` | enum com uma `Image<P>` por formato de pixel (inclusive `Hsv8`, `Hsl8`, `HsvF32`, `HslF32`); `From<Image<P>>`, `as_image`, `into_image`, `as_bytes` |
| `FrameMeta` | `index`, `timestamp: Duration`, `source: Arc<str>`; `FrameMeta::new(index, source)` |

## Funções de I/O

| Função | Descrição |
|---|---|
| `read_frame(path, index) -> Result<Frame>` | decodifica uma imagem como frame `Rgb8` |
| `write_frame(frame, path, format) -> Result<()>` | grava frames `L8`, `La8`, `Rgb8`, `Rgba8` ou `L16` em PNG ou JPEG; os demais formatos dão `UnsupportedFormat` |

Ambas ficam em `plugins::io` e podem ser usadas sem montar um pipeline.

## Stages

Enum `Stage` (um `SystemSet`): `Input`, `PreProcess`, `Process`, `PostProcess`, `Output`, `Cleanup`; `Stage::ALL` na ordem de execução. `PipelineBuilder::add_system(stage, sistemas)` e `add_op(stage, kernel)` — ver [Pipeline e stages](../conceitos/pipeline-e-stages.md).

## Componentes produzidos pelos filtros

| Componente | Inserido por | Conteúdo |
|---|---|---|
| `GrayscaleTag` | `grayscale_system` | marcador |
| `SobelTag` | `sobel_system` | marcador |
| `SobelMap` | `sobel_system` | `magnitude: Image<L8>` |
| `Prediction` | `inference_system` | `scores`, `class_id`, `confidence` (valores vazios hoje) |

## Plugins

`IoPlugin`, `FiltersPlugin`, `MlPlugin` — ver [Plugins](../conceitos/plugins.md).
