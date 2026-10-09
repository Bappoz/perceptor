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
    cargo clippy -p perceptor-core --all-targets --no-default-features -- -D warnings

# Testes unitários, de integração e doctests
test *args:
    cargo test --workspace {{ args }}
    cargo test -q -p perceptor-core --no-default-features {{ args }}

# Rustdoc com links quebrados e docs faltantes tratados como erro
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Build de release do workspace
build:
    cargo build --workspace --release

# Testes sob Miri (requer `rustup +nightly component add miri`)
miri *args:
    # Stacked Borrows (modelo padrão, mais estrito) sobre o core sem rayon.
    cargo +nightly miri test -p perceptor-core --no-default-features {{ args }}
    # Com rayon, Tree Borrows: o crossbeam-epoch (dependência do rayon) viola
    # Stacked Borrows internamente, fora do código deste repositório. O pool
    # global do rayon nunca é encerrado, daí o `ignore-leaks`.
    MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-ignore-leaks" cargo +nightly miri test -p perceptor-core {{ args }}

# Benchmarks criterion
bench *args:
    cargo bench --workspace {{ args }}

# Cobertura de testes (requer cargo-llvm-cov); `just cov --html` gera relatório navegável
cov *args:
    cargo llvm-cov --workspace {{ args }}

# Licenças, advisories e fontes das dependências (requer cargo-deny)
deny:
    cargo deny check

# Gates na ordem: format → lint → test → doc → build
ci: fmt-check lint test doc build

# Instala as dependências do site de documentação
docs-install:
    pnpm --dir website install --frozen-lockfile

# Site de documentação em modo de desenvolvimento
docs-dev:
    pnpm --dir website start

# Build de produção do site (falha em link quebrado) + typecheck
docs-build:
    pnpm --dir website typecheck
    pnpm --dir website build
