# Perceptor

[![CI](https://github.com/Bappoz/perceptor/actions/workflows/ci.yml/badge.svg)](https://github.com/Bappoz/perceptor/actions/workflows/ci.yml)
[![Docs](https://github.com/Bappoz/perceptor/actions/workflows/docs.yml/badge.svg)](https://bappoz.github.io/perceptor/)

Computer vision in Rust: pure, optimizable kernels orchestrated by an ECS pipeline.

- **Pure kernels** — every algorithm is a function over image buffers, with no ECS, I/O or global state, so it can be tested, benchmarked and optimized in isolation.
- **ECS pipeline** — frames are entities, transformations are systems, intermediate results are components. Built on `bevy_ecs`.
- **Memory safety first** — `unsafe` is confined to isolated modules; `perceptor-core` forbids it.

> Status: early development (0.1). The API changes between pull requests.

## Quick start

```toml
[dependencies]
perceptor = { git = "https://github.com/Bappoz/perceptor" }
anyhow = "1"
```

```rust
use perceptor::pipeline::plugins::{filters::FiltersPlugin, io::IoPlugin};
use perceptor::prelude::*;

fn main() -> anyhow::Result<()> {
    let mut pipeline = Pipeline::builder()
        .add_plugin(IoPlugin {
            input_path: "input.png".into(),
            output_path: "output.png".into(),
            ..Default::default()
        })
        .add_plugin(FiltersPlugin {
            enable_grayscale: true,
            enable_sobel: false,
        })
        .build();

    pipeline.run()?;
    Ok(())
}
```

Runnable versions live in [`examples/`](examples):

```bash
cargo run --example grayscale -- input.png output.png
cargo run --example custom_plugin
```

## What works today

| Area | Available |
|---|---|
| Pipeline | `Pipeline`, `PipelineBuilder`, five ordered stages, `Plugin` trait |
| I/O | read one image from disk, write PNG/JPEG (`read_frame`, `write_frame`, `IoPlugin`) |
| Filters | BT.601 grayscale, 3×3 Sobel magnitude |
| Errors | typed `perceptor_core::Error` across kernels and I/O |

Everything else — own image type, SIMD, camera capture, ML inference, GPU — is planned as one issue per increment in the [roadmap](ROADMAP.md).

## Workspace

| Crate | Role |
|---|---|
| `perceptor` | facade re-exporting the layers and the prelude |
| `perceptor-core` | fundamental types (`Error`, `Pixel` and pixel formats; `Image<P>` in progress) |
| `perceptor-imgproc` | image processing kernels as pure functions |
| `perceptor-pipeline` | ECS pipeline, stages, plugins and systems |

## Development

Requires Rust 1.95+ and [`just`](https://github.com/casey/just).

```bash
just ci          # fmt-check, clippy -D warnings, tests, rustdoc, release build
just docs-dev    # documentation site (requires pnpm)
```

Documentation (Portuguese): <https://bappoz.github.io/perceptor/> · sources in [`website/`](website).
Contribution workflow: [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT.
