# Perceptor — Claude Context

## O que é este projeto

Biblioteca Rust de Computer Vision baseada em ECS (Entity Component System), inspirada na arquitetura do Bevy. O modelo central é: **frames de vídeo são Entidades**, **transformações são Sistemas**, **metadados de processamento são Componentes**.

## Estado atual

**Fase 0 completa** — scaffold implementado. A "casca" está pronta:
- Toda a estrutura de módulos existe com assinaturas corretas
- Sistemas ECS estão registrados mas com lógica marcada como `// TODO`
- O pipeline roda sem panics; sistemas de filtro são no-ops seguros
- CI configurado e funcional

**Próximo passo**: implementar Fase 1 do ROADMAP — começar por `convert_to_gray` em `src/plugins/filters/grayscale.rs`.

## Arquitetura em uma linha

```
IoPlugin (InputStage) → FiltersPlugin (ProcessStage) → MlPlugin (PostProcessStage) → OutputStage
```

## Mapa de arquivos críticos

| Arquivo                                    | Papel                                              |
|--------------------------------------------|----------------------------------------------------|
| `src/core/frame.rs`                        | Componente `Frame` = tensor `Array3<u8>` [H, W, C]|
| `src/core/pipeline.rs`                     | `Pipeline` + `PipelineBuilder` — ponto de entrada  |
| `src/core/schedule.rs`                     | Labels de stage: Input→Pre→Process→Post→Output     |
| `src/core/plugin.rs`                       | Trait `Plugin` que todos os plugins implementam    |
| `src/plugins/io/video_reader.rs`           | TODO: ler frames de disco/câmera/vídeo             |
| `src/plugins/filters/grayscale.rs`         | TODO: `convert_to_gray` BT.601 com rayon           |
| `src/plugins/filters/sobel.rs`             | TODO: `apply_sobel` convolução 3×3                 |
| `src/plugins/ml/inference.rs`              | TODO: integrar ort/tch-rs                          |
| `ARCHITECTURE.md`                          | Diagramas Mermaid do fluxo completo                |
| `ROADMAP.md`                               | Steps de implementação por fase                    |

## Convenções de código Rust neste projeto

### Componentes ECS
- Sempre `#[derive(Component)]` — nunca implemente `Component` manualmente
- Componentes marcadores (sem campos) usam sufixo `Tag`: `GrayscaleTag`, `SobelTag`
- Componentes com dados usam nome descritivo: `SobelMap`, `Prediction`

### Sistemas ECS
- Funções `snake_case` com sufixo `_system`: `grayscale_system`, `inference_system`
- Parâmetros em ordem: `Commands` → `Query` → `Res` → `ResMut`
- Sempre use `Query<..., Without<Tag>>` para evitar re-processamento de frames já transformados
- Use `trace!()` no início de cada sistema com `entity` e `frame.meta.index`

### Recursos ECS
- `#[derive(Resource)]` para estado global
- Sufixo `Config` para configuração: `IoConfig`, `ModelConfig`
- Sufixo `State` para estado mutável do runtime: `PipelineState`

### Tensores ndarray
- Convenção de eixos: sempre `[H, W, C]` (altura, largura, canais)
- Tipo padrão: `Array3<u8>` para pixels brutos; `Array3<f32>` para processamento normalizado
- `debug_assert_eq!(tensor.ndim(), 3)` no início de funções que recebem tensores
- Para paralelismo: `use rayon::prelude::*` e `.par_iter()` em linhas/colunas

### Tratamento de erros
- Use `anyhow::Result<T>` em funções de alto nível (IO, pipeline)
- Use `thiserror` para erros de domínio específicos da lib (criar `src/error.rs` quando necessário)
- Sistemas ECS **não retornam** `Result` — loguem com `warn!`/`error!` e retornem early

### Features
- `gpu`: qualquer código wgpu deve estar sob `#[cfg(feature = "gpu")]`
- `ml`: backends ONNX/PyTorch sob `#[cfg(feature = "ml")]`
- Código sem feature flags deve compilar em modo `default` (CPU-only)

## Como adicionar um novo filtro

1. Crie `src/plugins/filters/meu_filtro.rs`
2. Defina componente marcador `MeuFiltroTag` e (se aplicável) componente de resultado
3. Implemente `meu_filtro_system(query: Query<...>, commands: Commands)`
4. Exporte de `src/plugins/filters/mod.rs`
5. Adicione ao `FiltersPlugin::build()` com flag opcional
6. Escreva teste unitário com `Array3::from_shape_vec`

## Como adicionar um novo plugin

1. Crie `src/plugins/meu_plugin/mod.rs`
2. Implemente `struct MeuPlugin { ... }` e `impl Plugin for MeuPlugin`
3. Registre sistemas nos stages corretos via `builder.add_*_system(...)`
4. Exporte de `src/plugins/mod.rs`
5. Re-exporte em `src/prelude.rs`

## Dependências atuais e versões

```toml
bevy_ecs = "0.15"     # ECS engine standalone
ndarray   = "0.15"    # Tensores [H, W, C]
image     = "0.25"    # I/O de imagens
rayon     = "1.10"    # Paralelismo data-parallel
anyhow    = "1.0"     # Erros em aplicações
thiserror = "2.0"     # Erros em bibliotecas
tracing   = "0.1"     # Logging estruturado
```

## Contexto acadêmico

Este é um projeto universitário com foco em:
- Entender e aplicar o padrão ECS em domínio não-game
- Processamento paralelo de imagens em Rust
- Preparar integração com modelos de ML (ONNX/PyTorch) para visão computacional
