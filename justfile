# Fonte canônica dos comandos do projeto. `just --list` mostra as receitas.

set shell := ["bash", "-euo", "pipefail", "-c"]

# Lista as receitas disponíveis
default:
    @just --list

# Formata todo o workspace
fmt:
    cargo fmt --all

# Verifica formatação sem alterar arquivos
fmt-check:
    cargo fmt --all -- --check

# Clippy pedantic com warnings tratados como erro
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Testes unitários, de integração e doctests
test *args:
    cargo test --workspace {{ args }}

# Rustdoc com links quebrados e docs faltantes tratados como erro
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Build de release do workspace
build:
    cargo build --workspace --release

# Testes sob Miri (requer `rustup +nightly component add miri`)
miri *args:
    cargo +nightly miri test -p perceptor-core {{ args }}

# Benchmarks criterion
bench *args:
    cargo bench --workspace {{ args }}

# Gates na ordem: format → lint → test → doc → build
ci: fmt-check lint test doc build
