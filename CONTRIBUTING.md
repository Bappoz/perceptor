# Contribuindo

## Fluxo

1. **Uma issue por incremento.** O plano está em [`ROADMAP.md`](ROADMAP.md); cada item é uma issue com escopo e critérios de aceite.
2. **Uma branch por issue**, a partir da branch principal: `feat/<n>-<slug>`, `fix/…`, `refactor/…`, `docs/…`, `ci/…`, `chore/…`.
3. **Teste antes do código.** Escreva o teste que falha, depois a implementação.
4. **Docs no mesmo PR.** Se o comportamento público mudou, a página correspondente em `website/docs/` e o changelog mudam junto.
5. **PR referenciando a issue** (`Closes #n`), com o que foi verificado e o que ficou de fora.

## Gates

```bash
just ci           # fmt-check → clippy -D warnings → test → rustdoc → build
just docs-build   # quando tocar website/ ou examples/
```

O CI roda os mesmos comandos. PR com gate vermelho não é revisado.

## Convenções

- **Commits:** [Conventional Commits](https://www.conventionalcommits.org/), subject imperativo em inglês, até 72 caracteres.
- **Kernels** ficam em `perceptor-imgproc` como funções puras; sistemas ECS em `perceptor-pipeline` apenas os chamam.
- **Erros:** funções públicas retornam `perceptor_core::Result`. `unwrap`/`expect` em código de lib só com a invariante comentada.
- **`unsafe`:** proibido fora dos módulos designados; todo bloco leva `// SAFETY:` e tem uma versão segura de referência testada contra ele.
- **Performance:** mudança de performance vem com benchmark antes e depois no PR.
- **Dependências novas** precisam de justificativa no PR.
- **Exemplos de código na documentação** são importados de `examples/`, que o CI compila; não cole trechos soltos.
