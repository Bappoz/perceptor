---
title: Plugins
---

# Plugins

Um plugin agrupa sistemas e recursos relacionados e os registra no builder.

```rust
pub trait Plugin: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn dependencies(&self) -> &[&str] { &[] }
    fn build(&self, builder: &mut PipelineBuilder);
    fn finish(&self, _builder: &mut PipelineBuilder) {}
    fn cleanup(&self, _world: &mut World) {}
}
```

## Ciclo de vida

| Método | Quando é chamado | Ordem |
|---|---|---|
| `build` | em `add_plugin`, para registrar sistemas | de registro |
| `finish` | em `PipelineBuilder::build()`, depois da validação — é onde recursos são inseridos | de registro |
| `cleanup` | em `Pipeline::shutdown()` ou no `Drop` do pipeline | inversa à de registro |

`cleanup` roda uma única vez por plugin: chamar `shutdown()` de novo, ou deixar o pipeline sair de escopo depois, não o repete. Ele recebe o `World` ainda íntegro, então pode ler e remover os recursos que o plugin inseriu. A ordem inversa garante que um plugin seja encerrado antes daqueles registrados antes dele.

## Validação

`PipelineBuilder::build()` retorna `Result<Pipeline>` e falha antes de chamar qualquer `finish`:

| Erro | Causa |
|---|---|
| `Error::DuplicatePlugin { name }` | dois plugins com o mesmo `name()`; o segundo nem chega a passar por `build` |
| `Error::MissingPluginDependency { plugin, dependency }` | um nome em `dependencies()` não corresponde a nenhum plugin registrado |

```rust
impl Plugin for OverlayPlugin {
    fn name(&self) -> &str { "OverlayPlugin" }
    fn dependencies(&self) -> &[&str] { &["FiltersPlugin"] }
    // ...
}
```

A ordem de registro não importa para as dependências: `add_plugin(Overlay).add_plugin(Filters)` é válido. O que se garante é que, quando `finish` roda, todos os plugins declarados já passaram por `build`. Um pipeline que falha na validação não chama `cleanup`.

## Plugins incluídos

### `IoPlugin`

Lê uma imagem do disco em `Stage::Input`, escreve os frames em `Stage::Output` e os remove em `Stage::Cleanup`.

| Campo | Tipo | Descrição |
|---|---|---|
| `input_path` | `PathBuf` | arquivo de entrada |
| `output_path` | `PathBuf` | arquivo de saída |
| `format` | `ImageFormat` | `Png` (padrão) ou `Jpeg` |
| `keep_frames` | `bool` | mantém os frames no `World` após a saída (padrão `false`) |

O leitor abre `input_path` uma única vez e avisa o fim da fonte em `Sources`, o que encerra `run()` com `StopReason::EndOfStream`; o escritor salva o frame. Leitura de sequências e outras fontes estão no milestone M7.

### `FiltersPlugin`

| Construtor | Filtros registrados |
|---|---|
| `FiltersPlugin::all()` | escala de cinza e Sobel |
| `FiltersPlugin::none()` / `default()` | nenhum |

Os campos `enable_grayscale` e `enable_sobel` permitem escolher individualmente.

### `MlPlugin`

Apenas a estrutura: registra um sistema que anexa uma `Prediction` vazia. O backend de inferência é o milestone M10.
