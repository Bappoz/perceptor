---
title: perceptor-pipeline
---

# perceptor-pipeline

Orquestração ECS sobre `bevy_ecs`.

## Tipos principais

| Item | Descrição |
|---|---|
| `Pipeline` | dono do `World` e dos schedules; `tick()`, `run()`, `world()`, `world_mut()` |
| `PipelineBuilder` | `add_plugin`, `add_*_system`, `with_max_ticks`, `build` |
| `PipelineState` | recurso com `tick_count` e `should_stop` |
| `Plugin` | trait de extensão |
| `Frame` | componente com `meta: FrameMeta` e `data: Array3<u8>` |
| `FrameMeta` | `index`, `timestamp_us`, `source` |

## Stages

`InputStage`, `PreProcessStage`, `ProcessStage`, `PostProcessStage`, `OutputStage` — ver [Pipeline e stages](../conceitos/pipeline-e-stages.md).

## Componentes produzidos pelos filtros

| Componente | Inserido por | Conteúdo |
|---|---|---|
| `GrayscaleTag` | `grayscale_system` | marcador |
| `SobelTag` | `sobel_system` | marcador |
| `SobelMap` | `sobel_system` | `magnitude: Array3<u8>` |
| `Prediction` | `inference_system` | `scores`, `class_id`, `confidence` (valores vazios hoje) |

## Plugins

`IoPlugin`, `FiltersPlugin`, `MlPlugin` — ver [Plugins](../conceitos/plugins.md).
