# Perceptor — visão computacional em Rust: kernels puros + pipeline ECS

## Stack
Rust 1.95+ (workspace) · `bevy_ecs` 0.20 · `ndarray` 0.17 · `image` 0.25 · `rayon` · testes com `cargo test` · site Docusaurus 3 (pnpm) em `website/`

## Comandos
```bash
just ci           # fmt-check → clippy -D warnings → test → rustdoc → build
just test -p perceptor-imgproc   # testes de uma crate
just docs-build   # typecheck + build do site (falha em link quebrado)
just bench -p perceptor-imgproc --bench grayscale   # criterion; registrar antes/depois no PR e em performance/index.md
just docs-dev     # site em modo dev
```
Rodar `just ci` antes de concluir qualquer mudança; `just docs-build` se tocar `website/` ou `examples/`.

## Arquitetura
- `crates/perceptor-core` — tipos fundamentais (`Error`, `Pixel` e formatos, `Image<P>`, views). `forbid(unsafe_code)`.
- `crates/perceptor-imgproc` — kernels como funções puras. Não conhece ECS nem I/O.
- `crates/perceptor-pipeline` — `Frame` (`FrameData`: uma `Image<P>` por formato, acesso por `as_image::<P>()`), stages, plugins e sistemas que chamam os kernels.
- `crates/perceptor-testkit` — só testes (`publish = false`): padrões sintéticos, estratégias proptest, `assert_images_eq!`, golden PNG. `perceptor-core` não a usa (dependência circular duplicaria os tipos).
- `fuzz/` — alvos `cargo-fuzz` (fora do workspace, nightly); o corpo vive em `perceptor_testkit::fuzz`. `just fuzz <alvo> [segundos]`.
- `src/` — fachada `perceptor` (re-exports + prelude). `examples/` — exemplos embutidos no site.
- `website/docs/` — documentação; `ROADMAP.md` — plano, uma issue por chunk.

Dependência só em uma direção: pipeline → imgproc → core.

## Convenções (não-negociáveis)
- Algoritmo novo = função pura em `imgproc` + sistema fino em `pipeline`. Nunca lógica de pixel dentro de sistema. Kernel `Frame → Frame` entra com `builder.add_op(stage, kernel)`; sistema à mão só quando o resultado é outro componente.
- API pública retorna `perceptor_core::Result`; `anyhow` só em `examples/`.
- Sistema não engole erro com `warn!`: retorna `Result` via `fallible(...)` ou chama `PipelineErrors::report_frame`.
- Teste de sistema/plugin usa `perceptor_pipeline::testing::TestPipeline`, não `World` + `Schedule` montados à mão.
- Teste que falha antes da implementação; otimização sempre comparada à versão escalar de referência.
- Um chunk = uma issue = uma branch = um PR (`Closes #n`), com página de docs e changelog (`website/docs/projeto/changelog.md`) atualizados.
- Trecho de código nos docs vem de `examples/` via `raw-loader`, não colado.
- Commits: Conventional Commits. Docs e rustdoc em PT-BR; README em inglês.

## Armadilhas
- O pipeline usa o executor single-thread do `bevy_ecs` (os spans de stage dependem disso). Não ligar `multi_threaded` nem trocar o executor; paralelismo é dentro do kernel, com rayon.
- Teste que instala um subscriber `tracing` vai em binário de teste próprio (`tests/metrics_spans.rs`): o cache de interesse por callsite é global ao processo.
- Ops de `add_op` rodam na ordem de registro dentro do stage, mas sem ordem em relação a sistemas de `add_system` do mesmo stage. Teste que mede alocação usa alocador global contador e vai em binário próprio (`tests/ops_alloc.rs`).
- Fonte finita se registra em `Sources` no `Plugin::build` e chama `finish` ao acabar; é isso que encerra `run()`. As condições de parada são avaliadas só no fim do tick. Teste que envia sinal ao processo vai em binário próprio (`tests/stop_signal.rs`).
- `Commands` só são aplicados no ponto de sincronização seguinte (entre stages, ou entre sistemas ordenados). Sistema que depende de componente inserido por outro do mesmo stage precisa de `(a, b).chain()` — sem isso só vê no tick seguinte, e com `IoPlugin` o frame já foi despawnado.
- `#[derive(Component)]` fora das crates do workspace exige `bevy_ecs` como dependência direta.
- pnpm 11 falha com script de instalação não declarado: ver `website/pnpm-workspace.yaml` (`allowBuilds`).
- Links entre páginas do site usam a extensão real do arquivo (`.md` vs `.mdx`); o build quebra se errar.

## Evitar
- Dependência nova sem justificativa no PR; lib que resolve o chunk inteiro sem discutir o que se perde em aprendizado.
- `unsafe` fora de `imgproc::simd`/`perceptor-hw` (ainda não existem).
- Adiantar milestones: siga a ordem do `ROADMAP.md`.

## Estado
Fonte da verdade = git (`git log --oneline -5`, branch atual) e as issues/milestones do GitHub.
