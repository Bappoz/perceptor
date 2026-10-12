//! Kernels puros registrados com `add_op`: grayscale → blur, sem escrever sistema.
//!
//! ```text
//! cargo run --example ops
//! ```

use perceptor::imgproc::grayscale::convert_to_gray;
use perceptor::prelude::*;

/// Kernel in-place: média horizontal `[1 2 1] / 4`, com a borda replicada.
///
/// Função pura, sem ECS. Só precisa do pixel original à esquerda, que guarda
/// em `left`, e por isso não aloca.
fn blur_rows(mut image: ImageViewMut<'_, L8>) {
    for row in image.rows_mut() {
        let Some(first) = row.first() else { continue };
        let mut left = u16::from(first.luma());
        for x in 0..row.len() {
            let center = u16::from(row[x].luma());
            let right = u16::from(row.get(x + 1).unwrap_or(&row[x]).luma());
            let mean = (left + 2 * center + right + 2) / 4;
            row[x] = L8::new(u8::try_from(mean).unwrap_or(u8::MAX));
            left = center;
        }
    }
}

/// Marca os frames já borrados: sem ela, um frame que ficasse no `World`
/// seria borrado de novo a cada tick.
#[derive(Component, Default)]
struct Blurred;

fn main() -> anyhow::Result<()> {
    let mut builder = Pipeline::builder();
    builder
        // `ImageView<Rgb8> -> Result<Image<L8>>`: a saída substitui o frame.
        .add_op(Stage::Process, convert_to_gray)
        // `ImageViewMut<L8>`: altera o frame no lugar.
        .add_op(Stage::Process, blur_rows.tagged::<Blurred>());
    let mut pipeline = builder.build()?;

    // Um degrau vertical: preto à esquerda, branco à direita.
    let step = Image::from_fn(6, 2, |x, _| {
        let v = if x < 3 { 0 } else { 255 };
        Rgb8::new(v, v, v)
    })?;
    let frame = pipeline
        .world_mut()
        .spawn(Frame::new(FrameMeta::new(0, "sintético"), step))
        .id();

    // Dois ticks: a marcação impede o segundo blur.
    pipeline.tick()?;
    pipeline.tick()?;

    let world = pipeline.world();
    let row: Vec<u8> = world
        .get::<Frame>(frame)
        .ok_or_else(|| anyhow::anyhow!("o frame saiu do World"))?
        .as_image::<L8>()?
        .row(0)
        .iter()
        .map(|p| p.luma())
        .collect();
    println!("linha borrada: {row:?}");

    assert_eq!(row, [0, 0, 64, 191, 255, 255]);
    assert!(world.get::<Blurred>(frame).is_some());
    Ok(())
}
