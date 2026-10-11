---
title: perceptor-pipeline
---

# perceptor-pipeline

Orquestração ECS sobre `bevy_ecs`.

## Tipos principais

| Item | Descrição |
|---|---|
| `Pipeline` | dono do `World` e do schedule; `tick()`, `run()`, `shutdown()`, `errors()`, `metrics()`, `world()`, `world_mut()` |
| `PipelineBuilder` | `add_plugin`, `add_*_system`, `with_max_ticks`, `with_error_policy`, `build() -> Result<Pipeline>` |
| `PipelineState` | recurso com `tick_count` e `should_stop` |
| `ErrorPolicy` | `FailFast` (padrão), `SkipFrame`, `Continue` |
| `PipelineErrors` | recurso com os erros do tick: `report`, `report_frame`, `iter`, `len`, `total`, `policy` |
| `SystemError` | `system`, `frame: Option<Entity>`, `error` |
| `fallible(sistema)` | adapta um sistema que retorna `Result<()>` |
| `PipelineMetrics` | recurso (feature `metrics`): `stage(Stage)`, `tick()`, `frames_processed()`, `frames_dropped()`, `fps()` — ver [Métricas e tracing](../guias/metricas.mdx) |
| `Timing` | `last`, `max`, `total`, `runs`, `mean()` |
| `Plugin` | trait de extensão: `name`, `dependencies`, `build`, `finish`, `cleanup` |
| `Frame` | componente com `meta: FrameMeta` e `data: FrameData`; `as_image::<P>()`, `as_image_mut::<P>()`, `format()`, `width()`, `height()`, `channels()` |
| `FrameData` | enum com uma `Image<P>` por formato de pixel; `From<Image<P>>`, `as_image`, `into_image`, `as_bytes` |
| `FrameMeta` | `index`, `timestamp: Duration`, `source: Arc<str>`; `FrameMeta::new(index, source)` |

## Funções de I/O

| Função | Descrição |
|---|---|
| `read_frame(path, index) -> Result<Frame>` | decodifica uma imagem como frame `Rgb8` |
| `write_frame(frame, path, format) -> Result<()>` | grava frames `L8`, `La8`, `Rgb8`, `Rgba8` ou `L16` em PNG ou JPEG; os demais formatos dão `UnsupportedFormat` |

Ambas ficam em `plugins::io` e podem ser usadas sem montar um pipeline.

## Stages

Enum `Stage` (um `SystemSet`): `Input`, `PreProcess`, `Process`, `PostProcess`, `Output`, `Cleanup`; `Stage::ALL` na ordem de execução. `PipelineBuilder::add_system(stage, sistemas)` — ver [Pipeline e stages](../conceitos/pipeline-e-stages.md).

## Componentes produzidos pelos filtros

| Componente | Inserido por | Conteúdo |
|---|---|---|
| `GrayscaleTag` | `grayscale_system` | marcador |
| `SobelTag` | `sobel_system` | marcador |
| `SobelMap` | `sobel_system` | `magnitude: Image<L8>` |
| `Prediction` | `inference_system` | `scores`, `class_id`, `confidence` (valores vazios hoje) |

## Plugins

`IoPlugin`, `FiltersPlugin`, `MlPlugin` — ver [Plugins](../conceitos/plugins.md).
