---
sidebar_position: 1
title: Introdução
---

# Perceptor

Perceptor é uma biblioteca de visão computacional em Rust organizada em duas ideias:

1. **Kernels puros** — cada algoritmo (conversão de cor, filtro, detector) é uma função sobre buffers de imagem, sem estado global, sem I/O e sem ECS.
2. **Pipeline ECS** — frames são *entidades*, transformações são *sistemas* e resultados intermediários são *componentes*. O pipeline apenas orquestra os kernels.

Essa separação é o que permite testar, medir e otimizar cada algoritmo isoladamente, e depois compor tudo em um fluxo de vídeo.

## Estado atual

O projeto está na versão `0.1` e em desenvolvimento inicial. Hoje existem:

| Área | O que funciona |
|---|---|
| Pipeline | `Pipeline`, `PipelineBuilder`, seis stages ordenados, trait `Plugin` |
| I/O | leitura de uma imagem do disco e escrita em PNG/JPEG (`IoPlugin`) |
| Cor | escala de cinza BT.601/BT.709, RGB ↔ BGR ↔ RGBA, RGB ↔ HSV/HSL, YUV (I420, NV12, YUYV) → RGB, alpha, separação e junção de canais |
| Operações pontuais | limiarização global (5 tipos), Otsu e limiarização adaptativa (média, gaussiana), tabelas de consulta (gamma, ganho/offset, inversão; 8 e 16 bits) |
| Filtros | Sobel 3×3 (`FiltersPlugin`) |
| Erros | `perceptor_core::Error` tipado em kernels e I/O |
| ML | apenas a estrutura do plugin; não há backend de inferência |

Tudo o que ainda não existe está planejado no [roadmap](./projeto/roadmap.md), uma issue por incremento.

## Por onde começar

- [Instalação](./primeiros-passos/instalacao.md)
- [Primeiro pipeline](./primeiros-passos/primeiro-pipeline.mdx)
- [Arquitetura](./conceitos/arquitetura.md)
