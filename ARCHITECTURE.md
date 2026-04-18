# Perceptor — Architecture

## Vision

Perceptor é uma biblioteca de Computer Vision onde **frames de vídeo são Entidades** em um mundo ECS (Entity Component System). Transformações de imagem são **Sistemas** que leem e escrevem **Componentes** nessas entidades. Essa abordagem oferece:

- **Composabilidade**: filtros são independentes e combináveis sem herança
- **Testabilidade**: sistemas são funções puras testáveis isoladamente
- **Paralelismo implícito**: o scheduler do ECS paralleliza sistemas sem conflito de acesso
- **Extensibilidade**: novos filtros/backends são plugins, sem tocar no core

---

## Fluxo de um frame (pipeline completo)

```mermaid
flowchart TD
    subgraph INPUT["InputStage"]
        A[IoPlugin\nimage_reader_system] -->|spawn Frame entity| W[(ECS World)]
    end

    subgraph PRE["PreProcessStage"]
        W --> B[resize_system\nTODO]
        W --> C[normalize_system\nTODO]
    end

    subgraph PROCESS["ProcessStage"]
        B & C --> D[grayscale_system\n+ GrayscaleTag]
        D --> E[sobel_system\n+ SobelMap + SobelTag]
    end

    subgraph POST["PostProcessStage"]
        E --> F[inference_system\n+ Prediction]
    end

    subgraph OUTPUT["OutputStage"]
        F --> G[display_system\nTODO]
        F --> H[writer_system\nTODO]
    end

    style INPUT   fill:#1a3a5c,color:#fff
    style PRE     fill:#1a4a2e,color:#fff
    style PROCESS fill:#4a2e1a,color:#fff
    style POST    fill:#3a1a4a,color:#fff
    style OUTPUT  fill:#1a4a4a,color:#fff
```

---

## Modelo ECS aplicado a CV

```mermaid
classDiagram
    class Frame {
        +FrameMeta meta
        +Array3~u8~ data
        +height() usize
        +width() usize
        +channels() usize
    }

    class GrayscaleTag {
        <<marker component>>
    }

    class SobelMap {
        +Array3~u8~ magnitude
    }

    class SobelTag {
        <<marker component>>
    }

    class Prediction {
        +Vec~f32~ scores
        +usize class_id
        +f32 confidence
    }

    Frame --> GrayscaleTag : após grayscale_system
    Frame --> SobelMap     : após sobel_system
    Frame --> SobelTag     : após sobel_system
    Frame --> Prediction   : após inference_system
```

---

## Estrutura de Módulos

```mermaid
graph TD
    lib["lib.rs\n(crate root)"]
    prelude["prelude.rs\n(re-exports públicos)"]

    subgraph CORE["src/core/"]
        frame["frame.rs\nFrame + FrameMeta"]
        schedule["schedule.rs\nStageLabels"]
        plugin["plugin.rs\nPlugin trait"]
        pipeline["pipeline.rs\nPipeline + PipelineBuilder"]
    end

    subgraph PLUGINS["src/plugins/"]
        subgraph IO["plugins/io/"]
            io_mod["mod.rs — IoPlugin"]
            video_reader["video_reader.rs\nimage_reader_system"]
        end
        subgraph FILTERS["plugins/filters/"]
            filters_mod["mod.rs — FiltersPlugin"]
            gray["grayscale.rs\ngrayscale_system"]
            sobel["sobel.rs\nsobel_system"]
        end
        subgraph ML["plugins/ml/"]
            ml_mod["mod.rs — MlPlugin"]
            inference["inference.rs\ninference_system"]
        end
    end

    lib --> CORE
    lib --> PLUGINS
    lib --> prelude
    pipeline --> plugin
    pipeline --> schedule
    io_mod --> plugin
    filters_mod --> plugin
    ml_mod --> plugin
```

---

## Ciclo de vida do Pipeline

```mermaid
sequenceDiagram
    actor User
    participant PB as PipelineBuilder
    participant P  as Pipeline
    participant W  as ECS World
    participant S  as Schedules

    User->>PB: Pipeline::builder()
    User->>PB: .add_plugin(IoPlugin)
    User->>PB: .add_plugin(FiltersPlugin::all())
    User->>PB: .build()
    PB->>W: World::new()
    PB->>W: insert_resource(PipelineState)
    PB->>P: Pipeline { world, schedules }

    loop a cada tick
        User->>P: pipeline.run()
        P->>S: input_schedule.run(world)
        Note over W: Frame spawned como entidade
        P->>S: pre_process_schedule.run(world)
        P->>S: process_schedule.run(world)
        Note over W: +GrayscaleTag +SobelMap +SobelTag
        P->>S: post_process_schedule.run(world)
        Note over W: +Prediction
        P->>S: output_schedule.run(world)
        Note over W: frame consumido / escrito
        P->>W: check PipelineState.should_stop
    end
```

---

## Camadas da Arquitetura

| Camada        | Módulo           | Responsabilidade                              |
|---------------|------------------|-----------------------------------------------|
| **Core/ECS**  | `core/`          | World, Schedule, Frame entity, Plugin trait   |
| **I/O**       | `plugins/io/`    | Leitura de fontes, escrita de saídas          |
| **Filtros**   | `plugins/filters/` | Transformações clássicas (grayscale, Sobel) |
| **ML**        | `plugins/ml/`    | Inferência com ONNX/PyTorch                   |
| **GPU**       | *(futuro)*       | Compute shaders via `wgpu`                    |

---

## Decisões de Design

### Por que `bevy_ecs` e não um ECS próprio?
- ECS maduro, battle-tested, com scheduler paralelo embutido
- Sistema de plugins idêntico ao Bevy — reaproveitamos o padrão
- Separável do Bevy completo (`bevy_ecs` como dependência standalone)

### Por que `ndarray` e não `Vec<u8>` plano?
- Semântica dimensional explícita `[H, W, C]` — erros de shape em compile-time (com `ndarray`)
- Integração nativa com `rayon` via feature `rayon` do ndarray
- Conversão direta para tensores `tch-rs`/`ort` sem cópia

### Por que stages separados (Input/Process/Output) e não um schedule único?
- Garante ordem de dependência entre sistemas sem necessidade de `.after()`/`.before()` manual
- Facilita profiling por fase
- Permite futures stages assíncronos (ex: input assíncrono de câmera)
