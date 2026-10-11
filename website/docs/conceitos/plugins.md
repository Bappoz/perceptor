---
title: Plugins
---

# Plugins

Um plugin agrupa sistemas e recursos relacionados e os registra no builder.

```rust
pub trait Plugin: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn build(&self, builder: &mut PipelineBuilder);
    fn finish(&self, _builder: &mut PipelineBuilder) {}
    fn cleanup(&self, _builder: &mut PipelineBuilder) {}
}
```

| Método | Quando é chamado |
|---|---|
| `build` | em `add_plugin`, para registrar sistemas |
| `finish` | em `build()`, depois que todos os plugins foram adicionados — é onde recursos são inseridos |
| `cleanup` | ainda não é chamado pelo pipeline ([#41](https://github.com/Bappoz/perceptor/issues/41)) |

## Plugins incluídos

### `IoPlugin`

Lê uma imagem do disco no `InputStage`, escreve os frames no `OutputStage` e os remove no `CleanupStage`.

| Campo | Tipo | Descrição |
|---|---|---|
| `input_path` | `PathBuf` | arquivo de entrada |
| `output_path` | `PathBuf` | arquivo de saída |
| `format` | `ImageFormat` | `Png` (padrão) ou `Jpeg` |
| `keep_frames` | `bool` | mantém os frames no `World` após a saída (padrão `false`) |

O leitor abre `input_path` a cada tick; o escritor salva e sinaliza parada. Leitura de sequências e outras fontes estão no milestone M7.

### `FiltersPlugin`

| Construtor | Filtros registrados |
|---|---|
| `FiltersPlugin::all()` | escala de cinza e Sobel |
| `FiltersPlugin::none()` / `default()` | nenhum |

Os campos `enable_grayscale` e `enable_sobel` permitem escolher individualmente.

### `MlPlugin`

Apenas a estrutura: registra um sistema que anexa uma `Prediction` vazia. O backend de inferência é o milestone M10.
