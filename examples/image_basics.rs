//! Criação e acesso a uma `Image<P>`.
//!
//! ```text
//! cargo run --example image_basics
//! ```

use perceptor::core::{Error, Image, Rgb8, L8};

fn main() -> perceptor::core::Result<()> {
    // Gradiente horizontal 4×2 em tons de cinza.
    let gray = Image::from_fn(4, 2, |x, _y| {
        L8::new(u8::try_from(x * 85).unwrap_or(u8::MAX))
    })?;
    println!("{gray:?}");
    println!("linha 0: {:?}", gray.as_subpixels().get(..4));

    // Acesso por coordenada: `get` devolve Option, o índice entra em pânico fora dos limites.
    let mut rgb = Image::<Rgb8>::new(2, 2)?;
    rgb[(1, 0)] = Rgb8::new(255, 0, 0);
    assert_eq!(rgb.get(1, 0), Some(&Rgb8::new(255, 0, 0)));
    assert_eq!(rgb.get(2, 0), None);

    // O buffer começa em um endereço múltiplo de 64 bytes.
    let address = rgb.as_bytes().as_ptr().addr();
    assert!(address.is_multiple_of(Image::<Rgb8>::ALIGNMENT));

    // Dimensões inválidas são erro, não pânico.
    match Image::<Rgb8>::new(0, 10) {
        Err(Error::InvalidDimensions { width, height }) => println!("rejeitado: {width}×{height}"),
        other => println!("inesperado: {other:?}"),
    }
    Ok(())
}
