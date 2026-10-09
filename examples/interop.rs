//! Troca de dados com as crates `image` e `ndarray`.
//!
//! ```text
//! cargo run --example interop
//! ```

use image::{Rgb, RgbImage};
use perceptor::core::{Image, ImageView, Rect, Rgb8};

fn main() -> perceptor::core::Result<()> {
    // Um buffer da crate `image`, como o que sai de um decodificador.
    let decoded = RgbImage::from_fn(4, 3, |x, y| {
        Rgb([
            60 * u8::try_from(x).unwrap_or(0),
            100 * u8::try_from(y).unwrap_or(0),
            0,
        ])
    });

    // Emprestado como visão do Perceptor: nenhum byte é copiado.
    let view = ImageView::<Rgb8>::from_image_buffer(&decoded)?;
    assert_eq!(view.get(3, 2), Some(&Rgb8::new(180, 200, 0)));

    // Cópia para uma imagem alinhada a 64 bytes, pronta para os kernels.
    let image = Image::<Rgb8>::from_image_buffer(&decoded)?;

    // Como array [altura, largura, canais] do ndarray, também sem cópia —
    // inclusive para um recorte, que vira um array com passo entre linhas.
    let array = image.view().as_array3();
    assert_eq!(array.dim(), (3, 4, 3));
    let crop = image.roi(Rect::new(1, 1, 2, 2))?;
    println!("recorte como array:\n{}", crop.as_array3());

    // De volta para `image`, para salvar em disco com `save`.
    let encoded = image.to_image_buffer()?;
    assert_eq!(encoded, decoded);
    Ok(())
}
