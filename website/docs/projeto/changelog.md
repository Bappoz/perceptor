---
title: Changelog
---

# Changelog

Mudanças por milestone, da mais recente para a mais antiga.

## M1 — Core de imagem (em andamento)

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
