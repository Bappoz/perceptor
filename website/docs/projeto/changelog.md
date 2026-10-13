---
title: Changelog
---

# Changelog

Mudanças por milestone, da mais recente para a mais antiga.

## M4 — Filtros espaciais (em andamento)

- `imgproc::convolve`: `Kernel` (lados ímpares, âncora no centro, `from_rows` e `flipped`), `correlate2d` e `convolve2d` com `BorderMode`, sobre qualquer formato de `u8`, `u16` ou `f32`. O formato de saída é escolhido na chamada: inteiro arredonda e satura, `f32` guarda o sinal. É a referência escalar dos filtros lineares, conferida com o `filter2D` do OpenCV ([#59](https://github.com/Bappoz/perceptor/issues/59)).

## M3 — Cor e operações pontuais

- `imgproc::range`: `in_range(view, lower, upper)` devolve uma máscara `L8` (255/0) dos pixels com `lower ≤ canal ≤ upper` em todos os canais, com limites inclusivos dados como um pixel do mesmo formato. Em HSV/HSL, `lower.h > upper.h` é a faixa que atravessa o zero do matiz ([#58](https://github.com/Bappoz/perceptor/issues/58)).
- `imgproc::arith`: `add`, `sub`, `absdiff`, `add_weighted` e `bitwise_and`/`or`/`xor`/`not`, sobre `u8`, `u16` e (exceto bit a bit) `f32`, com saturação nos inteiros e máscara opcional; fora da máscara vale o primeiro operando. Operandos ou máscara de outras dimensões dão `ShapeMismatch` ([#57](https://github.com/Bappoz/perceptor/issues/57)).
- `imgproc::convert`: `convert_scale` (`α·v + β` com saturação), `convert_depth` (`u8`/`u16`/`f32` pela faixa nominal; ida e volta `u8` → `f32` → `u8` exata nos 256 níveis), `normalize_minmax` (intervalo global de todos os canais), `channel_mean_std` e `normalize_mean_std` (por canal, em tons nominais). Origem e destino precisam ter o mesmo número de canais ([#56](https://github.com/Bappoz/perceptor/issues/56)).
- `imgproc::clahe`: `clahe(view, clip_limit, (colunas, linhas))` — histograma por tile cortado em `clip_limit` vezes a altura uniforme, excesso redistribuído como no OpenCV e interpolação bilinear (Q16) entre as tabelas dos quatro tiles vizinhos. Novo erro `InvalidParameter { name, reason }` para grade vazia ou maior que a imagem e `clip_limit` não positivo ([#55](https://github.com/Bappoz/perceptor/issues/55)).
- `imgproc::histogram`: `histogram` (um canal) e `histograms` (todos, em uma passada) com máscara opcional, `cumulative`, `equalization_lut` e `equalize`. A equalização é uma `Lut<u8>` calculada da CDF, com arredondamento para cima na metade, o menor nível indo a `0` e o maior a `255`; histograma de um só nível ou vazio dá a identidade ([#54](https://github.com/Bappoz/perceptor/issues/54)).
- `imgproc::lut`: `Lut<u8>` e `Lut<u16>` com `identity`, `invert`, `gamma`, `gain_offset` e `from_fn`; `apply_lut`, `apply_lut_in_place` e `apply_lut_per_channel` sobre qualquer formato de 8 ou 16 bits. Arredondamento para o mais próximo (metade para longe de zero) e saturação em `[0, máx]` ([#53](https://github.com/Bappoz/perceptor/issues/53)).
- `imgproc::adaptive`: `adaptive_threshold` e `adaptive_threshold_inv` comparam cada pixel com a média local da janela `block_size × block_size` menos `c`, com `AdaptiveMethod::{Mean, Gaussian}`. A média usa somas por prefixo (O(1) por pixel) e a gaussiana um kernel separável em Q16 que soma exatamente 2¹⁶; em ambos a comparação é exata, em inteiros, com borda replicada. `block_size` par ou menor que 3 dá o novo erro `InvalidWindowSize` ([#52](https://github.com/Bappoz/perceptor/issues/52)).
- `imgproc::threshold`: `threshold` e `threshold_in_place` com `ThresholdType::{Binary, BinaryInv, Trunc, ToZero, ToZeroInv}`, e limiar automático de Otsu (`otsu_level`, `otsu_from_histogram`, `threshold_otsu`), que devolve o meio do empate quando há um vão entre os modos ([#51](https://github.com/Bappoz/perceptor/issues/51)).
- `imgproc::yuv`: `i420_to_rgb`, `nv12_to_rgb` e `yuyv_to_rgb` sobre visões de bytes do buffer original, com `YuvSpec` para matriz (BT.601/BT.709) e faixa (limitada/completa); ponto fixo Q14 a um nível da referência em `f64`. Dimensões ímpares dão o novo erro `OddDimensions` ([#50](https://github.com/Bappoz/perceptor/issues/50)).
- `imgproc::hsv`: `rgb_to_hsv`, `hsv_to_rgb`, `rgb_to_hsl` e `hsl_to_rgb`, em 8 bits e em `f32`. Quatro formatos de pixel novos em `perceptor-core` — `Hsv8`, `Hsl8` (matiz em `0..180`), `HsvF32`, `HslF32` (matiz em graus) — aceitos também pelo `Frame`. A ida e volta em 8 bits erra até 4 níveis em HSV e 5 em HSL, medidos sobre todos os valores RGB ([#49](https://github.com/Bappoz/perceptor/issues/49)).
- `imgproc::channels`: `rgb_to_bgr`/`bgr_to_rgb`, `add_alpha`/`drop_alpha`, `bgr_to_rgba`/`rgba_to_bgr`, `swap_channels` no lugar, `extract_channel`, `split_channels` e `merge_channels`, com round-trip exato e um golden por conversão. Novo erro `ChannelOutOfRange` ([#48](https://github.com/Bappoz/perceptor/issues/48)).
- Escala de cinza em ponto fixo Q15 com arredondamento, pesos `LumaWeights::{Bt601, Bt709}` (`convert_to_gray_with`), versão de referência em `f64` e paralelismo por faixas de linhas só acima de 65 536 pixels. O resultado de `convert_to_gray` passa a ser arredondado em vez de truncado, então alguns pixels sobem um nível. Primeiro benchmark `criterion` do projeto ([#47](https://github.com/Bappoz/perceptor/issues/47)).

## M2 — Pipeline v2

- `PipelineBuilder::add_op`: registra um kernel puro como sistema a partir da assinatura dele (`ImageView<A> -> Result<Image<B>>` ou in-place sobre `ImageViewMut<P>`), em ordem de registro dentro do stage e com marcação opcional por componente (`Op::tagged`) ([#46](https://github.com/Bappoz/perceptor/issues/46)).
- Condições de parada: `run()` devolve um `StopReason`; fontes finitas avisam o fim no recurso `Sources`; limites `with_max_frames` e `with_max_duration` somam-se a `with_max_ticks`; `StopHandle` para parar de outra thread e `stop_on_ctrl_c()` atrás da feature `ctrlc`. `PipelineState::should_stop` deixa de ser um campo (use `request_stop()`), e o `IoPlugin` passa a ler o arquivo uma única vez em vez de a cada tick ([#45](https://github.com/Bappoz/perceptor/issues/45)).
- `testing::TestPipeline`: harness headless que injeta frames, roda ticks e verifica componentes por entidade; os testes de grayscale e Sobel foram reescritos sobre ele ([#44](https://github.com/Bappoz/perceptor/issues/44)).
- `PipelineMetrics`: latência por stage e por tick, fps e frames descartados; spans `tracing` `tick` e `stage`. Tudo atrás da feature `metrics` (padrão), mais `trace-systems` para um span por sistema. O pipeline passa a fixar o executor single-thread do `bevy_ecs` ([#43](https://github.com/Bappoz/perceptor/issues/43)).
- `Frame` multi-formato: `data` passa de `Array3<u8>` para `FrameData`, com acesso tipado `as_image::<P>()` que retorna `FormatMismatch` em vez de entrar em pânico. `FrameMeta` usa `Arc<str>` e `Duration`. Os kernels `convert_to_gray` e `apply_sobel` operam sobre `ImageView`/`Image`, o escritor grava `La8` e `L16`, e `ndarray` sai de `perceptor-imgproc` e `perceptor-pipeline` ([#42](https://github.com/Bappoz/perceptor/issues/42)).
- Contrato de `Plugin` cumprido: `PipelineBuilder::build()` passa a retornar `Result` e rejeita plugin duplicado ou dependência ausente (`Plugin::dependencies`); `Plugin::cleanup` recebe `&mut World` e é chamado em `Pipeline::shutdown()` ou no `Drop`, em ordem inversa de registro ([#41](https://github.com/Bappoz/perceptor/issues/41)).
- Erros de sistemas deixam de ser engolidos: recurso `PipelineErrors`, adaptador `fallible` e `ErrorPolicy` (`FailFast`, `SkipFrame`, `Continue`). O padrão é `FailFast`, então `tick()`/`run()` agora retornam `Err` onde antes só havia um `warn!`; `image_reader_system` passa a retornar `Result` ([#40](https://github.com/Bappoz/perceptor/issues/40)).
- Schedule único: `Stage` vira um `SystemSet` com seis variantes encadeadas e `add_system(Stage::X, …)`; grayscale e Sobel passam a rodar no mesmo tick. Os tipos `InputStage`…`CleanupStage` e `STAGE_ORDER` foram removidos ([#39](https://github.com/Bappoz/perceptor/issues/39)).
- `CleanupStage` e `despawn_processed_frames_system`: o `IoPlugin` remove cada frame depois da saída; `keep_frames: true` mantém o comportamento anterior ([#12](https://github.com/Bappoz/perceptor/issues/12)).

## M1 — Core de imagem

- Fuzzing de `perceptor-core` com `cargo-fuzz`: construtores, geometria de visões/ROI/bordas e interop com `ndarray` e `image`, com oráculos independentes e smoke test no CI ([#38](https://github.com/Bappoz/perceptor/issues/38)).
- `perceptor-testkit`: padrões sintéticos, estratégias `proptest`, comparadores (diferença máxima, MSE, PSNR), `assert_images_eq!` com diff legível e golden images em PNG com regeneração por variável de ambiente ([#37](https://github.com/Bappoz/perceptor/issues/37)).
- `BufferPool`: reutilização de buffers por `(dimensões, formato)` com devolução no `Drop`, limite de memória ociosa e uso concorrente ([#36](https://github.com/Bappoz/perceptor/issues/36)).
- Interoperabilidade com `image::ImageBuffer` e `ndarray::ArrayView3`, sem cópia nos dois sentidos quando o layout permite ([#35](https://github.com/Bappoz/perceptor/issues/35)).
- `BorderMode` com cinco modos de extrapolação na convenção do OpenCV, acesso com borda e linha estendida ([#34](https://github.com/Bappoz/perceptor/issues/34)).
- Iteradores por linha e por pixel, e versões paralelas por linha e por faixa sobre rayon ([#33](https://github.com/Bappoz/perceptor/issues/33)).
- `ImageView`/`ImageViewMut`: visões sem cópia com stride arbitrário, recortes (`roi`) e divisão em faixas mutáveis disjuntas ([#32](https://github.com/Bappoz/perceptor/issues/32)).
- `Image<P>`: imagem dona do buffer, alinhada a 64 bytes, com construtores falíveis ([#31](https://github.com/Bappoz/perceptor/issues/31)).
- Trait `Pixel` e oito formatos de pixel tipados com casts sem cópia entre fatias de pixels e de subpixels ([#30](https://github.com/Bappoz/perceptor/issues/30)).

## M0 — Fundação

- `cargo-deny` no CI: licenças, advisories e origem das dependências ([#21](https://github.com/Bappoz/perceptor/issues/21)).
- Cobertura de testes medida no CI com `cargo-llvm-cov` ([#29](https://github.com/Bappoz/perceptor/issues/29)).
- Miri no CI para `perceptor-core` ([#20](https://github.com/Bappoz/perceptor/issues/20)).
- `bevy_ecs` 0.20, `ndarray` 0.17 e MSRV 1.95; `wgpu` removido até o milestone de GPU ([#25](https://github.com/Bappoz/perceptor/issues/25)).
- README, `CONTRIBUTING.md` e `CLAUDE.md` do projeto ([#28](https://github.com/Bappoz/perceptor/issues/28)).
- Tipo de erro próprio (`perceptor_core::Error`); kernels e I/O deixam de entrar em pânico em entrada inválida e `anyhow` sai da API pública ([#24](https://github.com/Bappoz/perceptor/issues/24)).
- Site e rustdoc publicados no GitHub Pages a cada merge ([#27](https://github.com/Bappoz/perceptor/issues/27)).
- Site de documentação com Docusaurus ([#26](https://github.com/Bappoz/perceptor/issues/26)).
- CI executando formatação, clippy, testes, rustdoc e MSRV em todo PR ([#19](https://github.com/Bappoz/perceptor/issues/19)).
- `justfile` com as receitas de desenvolvimento ([#18](https://github.com/Bappoz/perceptor/issues/18)).
- Clippy pedantic sem warnings ([#22](https://github.com/Bappoz/perceptor/issues/22)).
- Testes fora da API pública e escrevendo em diretórios temporários ([#23](https://github.com/Bappoz/perceptor/issues/23)).
- Workspace dividido em `perceptor-core`, `perceptor-imgproc` e `perceptor-pipeline` ([#17](https://github.com/Bappoz/perceptor/issues/17)).

## Antes do M0

- `Pipeline` com cinco stages, `IoPlugin` para uma imagem, escala de cinza BT.601 e Sobel 3×3.
