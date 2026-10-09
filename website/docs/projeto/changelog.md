---
title: Changelog
---

# Changelog

Mudanças por milestone, da mais recente para a mais antiga.

## M0 — Fundação (em andamento)

- Site de documentação com Docusaurus ([#26](https://github.com/Bappoz/perceptor/issues/26)).
- CI executando formatação, clippy, testes, rustdoc e MSRV em todo PR ([#19](https://github.com/Bappoz/perceptor/issues/19)).
- `justfile` com as receitas de desenvolvimento ([#18](https://github.com/Bappoz/perceptor/issues/18)).
- Clippy pedantic sem warnings ([#22](https://github.com/Bappoz/perceptor/issues/22)).
- Testes fora da API pública e escrevendo em diretórios temporários ([#23](https://github.com/Bappoz/perceptor/issues/23)).
- Workspace dividido em `perceptor-core`, `perceptor-imgproc` e `perceptor-pipeline` ([#17](https://github.com/Bappoz/perceptor/issues/17)).

## Antes do M0

- `Pipeline` com cinco stages, `IoPlugin` para uma imagem, escala de cinza BT.601 e Sobel 3×3.
