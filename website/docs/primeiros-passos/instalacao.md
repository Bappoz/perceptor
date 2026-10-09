---
title: Instalação
---

# Instalação

## Requisitos

- Rust **1.88** ou mais recente (`rustup update stable`).

## Adicionando ao projeto

Perceptor ainda não está publicado no crates.io. Use a dependência por git:

```toml title="Cargo.toml"
[dependencies]
perceptor = { git = "https://github.com/Bappoz/perceptor" }
anyhow = "1"
# Necessário apenas para declarar seus próprios componentes com #[derive(Component)]
bevy_ecs = "0.15"
```

:::note
O derive `Component` gera código que referencia a crate `bevy_ecs` pelo nome, por isso ela precisa estar no seu `Cargo.toml` na mesma versão usada pelo Perceptor (`0.15`).
:::

## Compilando a partir do código

```bash
git clone https://github.com/Bappoz/perceptor
cd perceptor
cargo test --workspace
```

O repositório usa [`just`](https://github.com/casey/just) como fonte dos comandos de desenvolvimento:

| Comando | O que faz |
|---|---|
| `just fmt` | formata o workspace |
| `just lint` | clippy pedantic com warnings como erro |
| `just test` | testes unitários, de integração e doctests |
| `just doc` | rustdoc com links quebrados como erro |
| `just ci` | todos os gates, na ordem do CI |
| `just docs-dev` | este site em modo de desenvolvimento |
