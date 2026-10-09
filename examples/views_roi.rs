//! Visões sem cópia: recorte de uma imagem e buffer externo com preenchimento.
//!
//! ```text
//! cargo run --example views_roi
//! ```

use perceptor::core::{Image, ImageView, Rect, Rgb8, L8};

fn main() -> perceptor::core::Result<()> {
    let mut image = Image::<L8>::new(6, 4)?;

    // Recorte mutável 2×2 a partir de (2, 1): escreve direto no buffer da imagem.
    image.roi_mut(Rect::new(2, 1, 2, 2))?.fill(L8::new(255));
    for y in 0..image.height() {
        println!(
            "{:?}",
            image.row(y).iter().map(|p| p.luma()).collect::<Vec<_>>()
        );
    }

    // Duas metades mutáveis disjuntas — cada uma poderia ir para uma thread.
    let (mut top, mut bottom) = image.view_mut().split_at_row(2)?;
    top.row_mut(0)[0] = L8::new(1);
    bottom.row_mut(0)[0] = L8::new(2);

    // Buffer externo: RGB com 2 pixels por linha (6 bytes) e linhas de 8 bytes.
    let camera = [10u8, 20, 30, 40, 50, 60, 0, 0, 11, 21, 31, 41, 51, 61];
    let frame = ImageView::<Rgb8>::new(&camera, 2, 2, 8)?;
    println!("linha 1 da câmera: {:?}", frame.row(1));

    // Só aqui há cópia: para uma imagem compacta e alinhada.
    let owned = frame.to_image()?;
    assert_eq!(owned.as_subpixels().len(), 12);

    // Região fora da imagem é erro.
    println!("{}", image.roi(Rect::new(5, 0, 2, 1)).unwrap_err());
    Ok(())
}
