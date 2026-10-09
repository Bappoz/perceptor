//! Lê uma imagem, converte para escala de cinza e salva o resultado.
//!
//! ```text
//! cargo run --example grayscale -- entrada.png saida.png
//! ```

use perceptor::pipeline::plugins::{filters::FiltersPlugin, io::IoPlugin};
use perceptor::prelude::*;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        anyhow::bail!("uso: grayscale <entrada> <saida.png>");
    };

    let mut pipeline = Pipeline::builder()
        .add_plugin(IoPlugin {
            input_path: input.into(),
            output_path: output.into(),
            ..Default::default()
        })
        .add_plugin(FiltersPlugin {
            enable_grayscale: true,
            enable_sobel: false,
        })
        .build();

    // O IoPlugin lê um frame por tick e sinaliza parada após escrever a saída.
    pipeline.run()?;
    Ok(())
}
