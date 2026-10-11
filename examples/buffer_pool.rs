//! Reutilização de buffers entre frames com `BufferPool`.
//!
//! ```text
//! cargo run --example buffer_pool
//! ```

use perceptor::core::{BufferPool, Rgb8};

fn main() -> perceptor::core::Result<()> {
    // Retém no máximo 4 MiB de buffers ociosos.
    let pool = BufferPool::new(4 << 20);

    for frame_index in 0..100_u8 {
        // Mantém o conteúdo do frame anterior; `acquire_zeroed` zera.
        let mut frame = pool.acquire::<Rgb8>(640, 480)?;
        frame.as_pixels_mut().fill(Rgb8::new(frame_index, 0, 0));
        // `frame` volta ao pool aqui, no fim da iteração.
    }

    let stats = pool.stats();
    println!(
        "alocações: {}, reaproveitamentos: {}, bytes ociosos: {}",
        stats.allocations, stats.reuses, stats.idle_bytes
    );
    assert_eq!(stats.allocations, 1);
    Ok(())
}
