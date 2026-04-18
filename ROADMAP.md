# Perceptor — Roadmap de Implementação

> Status: `[ ]` pendente · `[~]` em progresso · `[x]` concluído

---

## Fase 0 — Scaffold (atual)
- [x] Workspace Rust com dependências essenciais
- [x] Estrutura de módulos `core/` e `plugins/`
- [x] Trait `Plugin` e `PipelineBuilder`
- [x] Componente `Frame` com `Array3<u8>`
- [x] Stage labels (`InputStage` → `OutputStage`)
- [x] CI via GitHub Actions (check + test + fmt)
- [x] `ARCHITECTURE.md` com diagramas Mermaid

---

## Fase 1 — Core funcional

### 1.1 Frame I/O
- [ ] `image_reader_system`: ler imagem estática com `image::open`
- [ ] `image_writer_system`: salvar frame como PNG/JPEG em disco
- [ ] `IoConfig` como recurso ECS configurável
- [ ] Teste de integração: ler imagem → spawnar entidade → verificar shape

### 1.2 Filtros clássicos
- [ ] `grayscale_system`: conversão BT.601 com `ndarray` + `rayon`
  - Implementar `convert_to_gray(&Array3<u8>) -> Array3<u8>`
  - Paralelizar por linha com `rayon::par_iter`
- [ ] `sobel_system`: convolução Sobel 3×3 com zero-padding
  - Implementar `apply_sobel(&Array3<u8>) -> Array3<u8>`
  - Usar `ndarray::s![]` para janelas deslizantes
- [ ] Testes unitários para cada filtro com imagens sintéticas
- [ ] Benchmark com `criterion`: throughput frames/sec por filtro

### 1.3 Pipeline completo (end-to-end)
- [ ] Teste de integração: imagem PNG → grayscale → Sobel → arquivo PNG
- [ ] `PipelineState.tick_count` incrementado a cada tick
- [ ] Condição de parada: `should_stop = true` após N ticks (modo batch)

---

## Fase 2 — Streaming de vídeo

### 2.1 Leitura de vídeo
- [ ] Dependência: `ffmpeg-next` ou `opencv` (feature flag)
- [ ] `VideoReader`: struct com estado de decodificação de vídeo
- [ ] `video_reader_system`: spawna um `Frame` por tick do vídeo
- [ ] `FrameMeta.timestamp_us` calculado a partir do PTS do vídeo

### 2.2 Leitura de câmera
- [ ] Dependência: `nokhwa` (abstração cross-platform de webcam)
- [ ] `CameraPlugin`: leitor de câmera como fonte alternativa ao `IoPlugin`
- [ ] Configuração de resolução, FPS e formato de pixel

### 2.3 Performance
- [ ] Perfilagem com `cargo flamegraph` ou `perf`
- [ ] Evitar alocações por frame: pool de `Array3` reutilizáveis
- [ ] Benchmark: latência total source → output em 1080p/30fps

---

## Fase 3 — Machine Learning

### 3.1 Backend ONNX Runtime
- [ ] Dependência: `ort` com feature `cuda` opcional
- [ ] `OnnxModel`: wrapper que carrega modelo e expõe `run(&Array3) -> Vec<f32>`
- [ ] `inference_system`: pré-processamento (normalizar `[0,1]`, reshape NCHW)
- [ ] Pós-processamento: softmax para classificação
- [ ] Teste: rodar MobileNetV2 em ONNX sobre imagem de teste

### 3.2 Detecção de Objetos
- [ ] Componente `BoundingBox { x, y, w, h, class_id, confidence }`
- [ ] NMS (Non-Maximum Suppression) como sistema no `PostProcessStage`
- [ ] Suporte a YOLO v8 ONNX export
- [ ] `DrawDetectionsPlugin`: renderiza bounding boxes no frame de saída

### 3.3 Backend PyTorch (opcional)
- [ ] Feature flag `ml-torch`
- [ ] Dependência: `tch` (tch-rs) com LibTorch
- [ ] `TorchModel`: wrapper para TorchScript (`.pt`)

---

## Fase 4 — Aceleração GPU

### 4.1 wgpu Compute
- [ ] Feature flag `gpu` (já declarada no `Cargo.toml`)
- [ ] `GpuContext`: inicialização de `wgpu::Device` e `wgpu::Queue`
- [ ] `GpuFrame`: componente alternativo com dados em buffer de GPU
- [ ] Compute shader WGSL para `grayscale`
- [ ] Compute shader WGSL para `sobel`
- [ ] Benchmark: GPU vs CPU em 4K frames

### 4.2 Pipeline híbrido CPU/GPU
- [ ] Sistema de migração: CPU `Array3` → GPU buffer (upload)
- [ ] Sistema de migração: GPU buffer → CPU `Array3` (readback)
- [ ] Critério automático de offload baseado em tamanho do frame

---

## Fase 5 — Ecossistema

### 5.1 Workspace multi-crate
- [ ] Extrair `perceptor-core` como crate separada
- [ ] Extrair `perceptor-plugins` como crate separada
- [ ] Publicar no crates.io com docs.rs

### 5.2 Python bindings
- [ ] Dependência: `pyo3`
- [ ] Expor `Pipeline`, `Frame`, `FiltersPlugin` para Python
- [ ] Wheel via `maturin`

### 5.3 Documentação
- [ ] Exemplos em `examples/`: grayscale, sobel, object detection
- [ ] Guia de "como escrever um Plugin custom"
- [ ] Badges de CI, cobertura de testes e docs.rs no README

---

## Dependências críticas (por fase)

| Fase | Crate              | Razão                              |
|------|--------------------|------------------------------------|
| 1    | `ndarray` + `rayon`| Tensores paralelos                 |
| 2    | `ffmpeg-next`      | Decodificação de vídeo             |
| 2    | `nokhwa`           | Captura de câmera                  |
| 3    | `ort`              | ONNX Runtime                       |
| 3    | `tch`              | PyTorch/TorchScript                |
| 4    | `wgpu`             | GPU compute (já opcional no Cargo) |
| 5    | `pyo3` + `maturin` | Python bindings                    |
