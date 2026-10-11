# Perceptor — visão computacional em Rust: kernels puros + pipeline ECS

## Stack
Rust 1.95+ (workspace) · `bevy_ecs` 0.20 · `ndarray` 0.17 · `image` 0.25 · `rayon` · testes com `cargo test` · site Docusaurus 3 (pnpm) em `website/`

## Comandos
```bash
just ci           # fmt-check → clippy -D warnings → test → rustdoc → build
just test -p perceptor-imgproc   # testes de uma crate
just docs-build   # typecheck + build do site (falha em link quebrado)
just docs-dev     # site em modo dev
```
Rodar `just ci` antes de concluir qualquer mudança; `just docs-build` se tocar `website/` ou `examples/`.

## Arquitetura
- `crates/perceptor-core` — tipos fundamentais (`Error`, `Pixel` e formatos, `Image<P>`, views). `forbid(unsafe_code)`.
- `crates/perceptor-imgproc` — kernels como funções puras. Não conhece ECS nem I/O.
- `crates/perceptor-pipeline` — `Frame`, stages, plugins e sistemas que chamam os kernels.
- `crates/perceptor-testkit` — só testes (`publish = false`): padrões sintéticos, estratégias proptest, `assert_images_eq!`, golden PNG. `perceptor-core` não a usa (dependência circular duplicaria os tipos).
- `fuzz/` — alvos `cargo-fuzz` (fora do workspace, nightly); o corpo vive em `perceptor_testkit::fuzz`. `just fuzz <alvo> [segundos]`.
- `src/` — fachada `perceptor` (re-exports + prelude). `examples/` — exemplos embutidos no site.
- `website/docs/` — documentação; `ROADMAP.md` — plano, uma issue por chunk.

Dependência só em uma direção: pipeline → imgproc → core.

## Convenções (não-negociáveis)
- Algoritmo novo = função pura em `imgproc` + sistema fino em `pipeline`. Nunca lógica de pixel dentro de sistema.
- API pública retorna `perceptor_core::Result`; `anyhow` só em `examples/`.
- Teste que falha antes da implementação; otimização sempre comparada à versão escalar de referência.
- Um chunk = uma issue = uma branch = um PR (`Closes #n`), com página de docs e changelog (`website/docs/projeto/changelog.md`) atualizados.
- Trecho de código nos docs vem de `examples/` via `raw-loader`, não colado.
- Commits: Conventional Commits. Docs e rustdoc em PT-BR; README em inglês.

## Armadilhas
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
