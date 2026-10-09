# Perceptor — Roadmap

Biblioteca de visão computacional em Rust: kernels puros e otimizados, orquestrados por um pipeline ECS.
Cada item abaixo é uma issue (um *chunk*): uma branch, um PR, testes escritos antes e a página de docs atualizada.

## Arquitetura alvo

```text
perceptor (fachada, features)
├─ perceptor-core      Image<P>, views/ROI, formatos de pixel, bordas, erro, pool
├─ perceptor-imgproc   kernels puros: cor, filtros, geometria, features (simd/ isola o unsafe)
├─ perceptor-pipeline  bevy_ecs: Frame, stages, plugins, erros, métricas
├─ perceptor-io        Source/Sink, imagem, sequência, vídeo, display
├─ perceptor-hw        V4L2 e integração de plataforma
├─ perceptor-ml        ONNX Runtime, pré/pós-processamento, NMS, tracking
├─ perceptor-gpu       wgpu compute (WGSL)
└─ perceptor-recipes   módulos prontos + CLI
```

## Princípios

- **Kernel puro primeiro**: toda operação existe como função sobre views, sem ECS; o pipeline só orquestra.
- **Referência escalar sempre**: cada otimização (SIMD, tiles, GPU) é provada equivalente à versão escalar por proptest.
- **`unsafe` confinado**: apenas em `perceptor-imgproc::simd` e `perceptor-hw`, com `// SAFETY:` e Miri no CI.
- **Medir antes de otimizar**: benchmark criterion antes/depois em todo PR de performance.
- **Ordem de execução**: M0 → M1 → M2 → M3 → M4 → M6 (parcial) → M5 → M7 → demais.

## M0 — Fundação

Workspace, tooling, CI, qualidade base e site de docs.

- [x] #17 chore(workspace): converter em workspace multi-crate (core, imgproc, pipeline + fachada)
- [x] #18 chore(tooling): justfile com receitas fmt, lint, test, miri, doc, bench e ci
- [x] #19 ci: pipeline de qualidade (fmt, clippy -D warnings, test, doc, MSRV)
- [x] #20 ci: Miri para perceptor-core e módulos com unsafe
- [x] #21 chore(deps): cargo-deny para licenças, advisories e duplicatas
- [x] #22 refactor: zerar warnings do clippy pedantic (74 hoje)
- [x] #23 refactor(test): remover `pub mod tests` da API e tirar fixtures/saídas de src/
- [x] #24 feat(core): PerceptorError com thiserror e alias Result
- [x] #25 chore(deps): atualizar bevy_ecs, ndarray e wgpu e fixar MSRV
- [x] #26 docs: scaffold Docusaurus em website/ com estrutura de documentação de lib
- [x] #27 ci(docs): build e deploy do Docusaurus no GitHub Pages
- [x] #28 docs: README vitrine, CLAUDE.md do projeto e CONTRIBUTING
- [x] #29 ci: cobertura de testes com cargo-llvm-cov

## M1 — Core de imagem

Image<P>, views, formatos de pixel, bordas, pool e infra de testes.

- [ ] #30 feat(core): trait Pixel e formatos L8, La8, Rgb8, Rgba8, Bgr8, L16, LF32, RgbF32
- [ ] #31 feat(core): Image<P> — buffer contíguo alinhado a 64 bytes com stride
- [ ] #32 feat(core): ImageView/ImageViewMut e ROI zero-copy
- [ ] #33 feat(core): iteradores por linha/pixel e divisão paralela por faixas
- [ ] #34 feat(core): BorderMode e acesso a pixel com borda
- [ ] #35 feat(core): interop com as crates image e ndarray por feature
- [ ] #36 feat(core): BufferPool para reutilização de buffers por frame
- [ ] #37 test(core): infraestrutura de proptest, golden images e geradores sintéticos
- [ ] #38 fuzz(core): harness cargo-fuzz para construtores e conversões de Image

## M2 — Pipeline v2

Schedule único, erros, plugins, Frame multi-formato, métricas.

- [ ] #12 feat: DespawnSystem — remover entidades Frame processadas após output
- [ ] #39 refactor(pipeline): schedule único com SystemSets ordenados
- [ ] #40 feat(pipeline): propagação de erros de sistemas com política configurável
- [ ] #41 feat(pipeline): deduplicação, dependências e cleanup de plugins
- [ ] #42 feat(pipeline): Frame multi-formato sobre Image<P>
- [ ] #43 feat(pipeline): métricas e spans de tracing por stage e sistema
- [ ] #44 feat(pipeline): harness de teste headless para sistemas e plugins
- [ ] #45 feat(pipeline): condições de parada (fim de fonte, limite de frames, sinal)
- [ ] #46 feat(pipeline): adaptador para registrar kernels puros como sistemas

## M3 — Cor e operações pontuais

Conversões de cor, threshold, LUT, histograma, aritmética.

- [ ] #47 perf(imgproc): grayscale em ponto fixo com pesos BT.601/BT.709 e arredondamento
- [ ] #48 feat(imgproc): conversões RGB↔BGR↔RGBA e troca/extração de canais
- [ ] #49 feat(imgproc): conversão RGB↔HSV/HSL
- [ ] #50 feat(imgproc): conversão YUV (YUYV, NV12, I420) → RGB
- [ ] #51 feat(imgproc): threshold binário, inverso, truncado e Otsu
- [ ] #52 feat(imgproc): threshold adaptativo (média e gaussiano)
- [ ] #53 feat(imgproc): LUT, gamma e brilho/contraste
- [ ] #54 feat(imgproc): histograma e equalização
- [ ] #55 feat(imgproc): CLAHE
- [ ] #56 feat(imgproc): normalize e convert_scale entre profundidades
- [ ] #57 feat(imgproc): operações aritméticas e lógicas com máscara
- [ ] #58 feat(imgproc): in_range e máscara por faixa de cor

## M4 — Filtros espaciais

Convolução, blur, gradientes, bordas, morfologia.

- [ ] #11 feat: SobelMap como output padrão — copiar magnitude para Frame.data
- [ ] #15 feat: GaussianBlurPlugin — kernel 3×3 para redução de ruído
- [ ] #59 feat(imgproc): convolução 2D genérica com BorderMode
- [ ] #60 feat(imgproc): filtro separável (linha/coluna) com buffer intermediário reutilizável
- [ ] #61 feat(imgproc): box blur por soma deslizante
- [ ] #62 refactor(imgproc): Sobel/Scharr sobre filtro separável com magnitude e orientação
- [ ] #63 feat(imgproc): filtro de mediana (3×3 por rede de ordenação, k×k por histograma)
- [ ] #64 feat(imgproc): filtro bilateral
- [ ] #65 feat(imgproc): Laplaciano e LoG
- [ ] #66 feat(imgproc): detector de bordas Canny
- [ ] #67 feat(imgproc): morfologia — erode, dilate, open, close, gradient, top-hat
- [ ] #68 feat(imgproc): imagem integral (soma e soma de quadrados)

## M5 — Geometria

Resize, warp, remap, pirâmides.

- [ ] #69 feat(imgproc): resize nearest e bilinear
- [ ] #70 feat(imgproc): resize bicúbico, Lanczos e area
- [ ] #71 feat(imgproc): crop, flip, rotate90, transpose e pad
- [ ] #72 feat(imgproc): warp afim
- [ ] #73 feat(imgproc): warp de perspectiva e homografia a partir de 4 pontos
- [ ] #74 feat(imgproc): remap genérico por mapas de coordenadas
- [ ] #75 feat(imgproc): pirâmides gaussiana e laplaciana
- [ ] #76 feat(imgproc): letterbox e resize preservando aspecto

## M6 — Performance

Benchmarks, SIMD, tiles, hot path sem alocação.

- [ ] #14 bench: pipeline_bench.rs — throughput de grayscale e sobel com criterion
- [ ] #77 perf(simd): infraestrutura de dispatch em runtime (AVX2, SSE4.1, NEON, escalar)
- [ ] #78 perf(simd): grayscale e conversões de cor
- [ ] #79 perf(simd): convolução separável, gaussian e box
- [ ] #80 perf(simd): threshold, aritmética saturada e absdiff
- [ ] #81 perf(simd): resize bilinear
- [ ] #82 perf: paralelismo por faixas/tiles com heurística de corte
- [ ] #83 perf: hot path sem alocação com teste de contagem de alocações
- [ ] #84 ci(bench): detecção de regressão de benchmark em PR
- [ ] #85 bench: comparativo com imageproc e OpenCV
- [ ] #86 docs(perf): guia de profiling com flamegraph e perf

## M7 — I/O e hardware

Sources/sinks, V4L2, vídeo, display, timestamps.

- [ ] #13 feat: suporte a glob de arquivos no IoPlugin — leitura sequencial de frames
- [ ] #87 feat(io): traits FrameSource e FrameSink
- [ ] #88 feat(io): decode/encode de imagem para Image<P> e writer com padrão de nome
- [ ] #89 feat(hw): V4L2 — enumeração de dispositivos, formatos e resoluções
- [ ] #90 feat(hw): captura V4L2 por streaming mmap (YUYV e MJPEG)
- [ ] #91 feat(hw): controles de câmera (exposição, ganho, foco, fps)
- [ ] #92 feat(io): captura em thread dedicada com canal limitado e política de backpressure
- [ ] #93 feat(io): decode de vídeo por feature `video`
- [ ] #94 feat(io): gravação de vídeo
- [ ] #95 feat(io): sink de display em janela
- [ ] #96 feat(io): relógio, timestamps e limitador de FPS
- [ ] #97 fuzz(io): fuzzing de decoders e parsers de formato
- [ ] #98 feat(io): fonte de rede (RTSP)

## M8 — Features e análise

Componentes conexos, contornos, cantos, descritores, fluxo óptico.

- [ ] #99 feat(imgproc): rotulação de componentes conexos com estatísticas
- [ ] #100 feat(imgproc): extração de contornos e aproximação poligonal
- [ ] #101 feat(imgproc): momentos, área, perímetro, bounding box e convex hull
- [ ] #102 feat(imgproc): detecção de cantos Harris e Shi-Tomasi
- [ ] #103 feat(imgproc): FAST e descritores ORB
- [ ] #104 feat(imgproc): matching de descritores (Hamming, ratio test, cross-check)
- [ ] #105 feat(imgproc): transformada de Hough para linhas e círculos
- [ ] #106 feat(imgproc): template matching (SSD e NCC)
- [ ] #107 feat(imgproc): fluxo óptico Lucas-Kanade piramidal
- [ ] #108 feat(imgproc): transformada de distância
- [ ] #109 feat(imgproc): subtração de fundo (média móvel e mistura de gaussianas)

## M9 — Overlay

Primitivas de desenho, texto, bbox, colormaps.

- [ ] #110 feat(overlay): primitivas de desenho — linha, retângulo, círculo, polígono
- [ ] #111 feat(overlay): texto com fonte bitmap embutida
- [ ] #112 feat(overlay): detecções, máscaras e keypoints sobre o frame
- [ ] #113 feat(overlay): colormaps (viridis, turbo, jet) e visualização de mapas escalares

## M10 — ML

ONNX Runtime, pré/pós-processamento, detecção, tracking.

- [ ] #114 feat(ml): backend ONNX Runtime com sessão e execution providers
- [ ] #115 feat(ml): pré-processamento para tensor (NCHW/NHWC, normalização)
- [ ] #116 feat(ml): classificação — softmax, top-k e componente Prediction
- [ ] #117 feat(ml): detecção de objetos — decode de saída estilo YOLO
- [ ] #118 feat(ml): IoU e Non-Maximum Suppression
- [ ] #119 feat(ml): tracking multi-objeto (SORT / ByteTrack)
- [ ] #120 feat(ml): segmentação — decode e redimensionamento de máscaras
- [ ] #121 feat(ml): inferência em lote e fora do tick
- [ ] #122 test(ml): modelos mínimos de fixture e testes determinísticos

## M11 — GPU

wgpu compute, kernels WGSL, offload híbrido.

- [ ] #123 feat(gpu): GpuContext — device, queue e seleção de adapter
- [ ] #124 feat(gpu): GpuImage — upload e readback
- [ ] #125 feat(gpu): kernels WGSL — grayscale, convolução e Sobel
- [ ] #126 feat(gpu): kernels WGSL — resize, threshold e conversão de cor
- [ ] #127 feat(gpu): offload híbrido CPU/GPU no pipeline
- [ ] #128 bench(gpu): CPU × GPU por kernel e resolução
- [ ] #129 ci(gpu): testes de GPU com backend de software

## M12 — Módulos prontos

Recipes de pipelines completos e CLI.

- [ ] #130 feat(recipes): detecção de bordas pronta para uso
- [ ] #131 feat(recipes): detecção de movimento
- [ ] #132 feat(recipes): câmera → detecção → overlay → display
- [ ] #133 feat(recipes): scanner de documento
- [ ] #134 feat(recipes): calibração de câmera e undistort
- [ ] #135 feat(cli): binário perceptor com subcomandos
- [ ] #136 feat(cli): pipeline declarativo em TOML

## M13 — Ecossistema

Bindings, publicação, plataformas alvo.

- [ ] #137 feat(python): bindings com pyo3 e interop numpy sem cópia
- [ ] #138 feat(ffi): API C com cbindgen
- [ ] #139 docs: galeria de exemplos em examples/
- [ ] #140 chore(release): publicação no crates.io, docs.rs e automação de release
- [ ] #141 docs: versionamento da documentação e tradução EN
- [ ] #142 feat(wasm): alvo WebAssembly e demo no site
- [ ] #143 feat(core): suporte a no_std + alloc
- [ ] #144 ci: cross-compilação aarch64 (Raspberry Pi / Jetson) com testes NEON
