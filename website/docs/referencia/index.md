---
title: Referência
---

# Referência

Visão geral da API pública de cada crate. A referência completa, item a item, é o [rustdoc publicado em `/api`](https://bappoz.github.io/perceptor/api/perceptor/), gerado a cada merge na branch principal. Para gerar localmente:

```bash
just doc
# abre target/doc/perceptor/index.html
```

| Crate | Conteúdo |
|---|---|
| [`perceptor-core`](./perceptor-core.md) | tipos fundamentais |
| [`perceptor-imgproc`](./perceptor-imgproc.md) | kernels de processamento |
| [`perceptor-pipeline`](./perceptor-pipeline.md) | pipeline ECS, stages e plugins |

A fachada `perceptor` reexporta as três como `perceptor::core`, `perceptor::imgproc` e `perceptor::pipeline`, além de `perceptor::prelude`.
