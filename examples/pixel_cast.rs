//! Visões tipadas sobre um buffer de bytes, sem cópia.
//!
//! ```text
//! cargo run --example pixel_cast
//! ```

use perceptor::core::{Pixel, PixelFormat, Rgb8};

fn main() -> perceptor::core::Result<()> {
    // Um buffer bruto como o que chega de um decodificador ou de uma câmera.
    let mut raw = vec![10u8, 20, 30, 40, 50, 60];

    // &mut [u8] → &mut [Rgb8]: mesmo endereço, sem cópia.
    let pixels = Rgb8::cast_slice_mut(&mut raw)?;
    pixels[1] = Rgb8::new(255, 0, 0);
    println!("{} pixels {:?}", pixels.len(), Rgb8::FORMAT);

    assert_eq!(raw, [10, 20, 30, 255, 0, 0]);
    assert_eq!(PixelFormat::Rgb8.bytes_per_pixel(), 3);

    // Comprimento que não fecha um número inteiro de pixels é erro, não pânico.
    let err = Rgb8::cast_slice(&raw[..5]).unwrap_err();
    println!("erro: {err}");
    Ok(())
}
