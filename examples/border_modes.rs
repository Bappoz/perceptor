//! Como cada `BorderMode` extrapola além das bordas.
//!
//! ```text
//! cargo run --example border_modes
//! ```

use perceptor::core::{BorderMode, Image, L8};

fn main() -> perceptor::core::Result<()> {
    // Uma linha "abcdefgh" como imagem 8×1.
    let image = Image::from_fn(8, 1, |x, _| L8::new(b'a' + u8::try_from(x).unwrap_or(0)))?;
    let view = image.view();

    let modes = [
        ("Constant('.')", BorderMode::Constant(L8::new(b'.'))),
        ("Replicate", BorderMode::Replicate),
        ("Reflect", BorderMode::Reflect),
        ("Reflect101", BorderMode::Reflect101),
        ("Wrap", BorderMode::Wrap),
    ];

    // O mesmo buffer é reutilizado para todas as linhas.
    let mut row = Vec::new();
    for (name, border) in modes {
        view.padded_row(0, 4, border, &mut row);
        let text: String = row.iter().map(|p| char::from(p.luma())).collect();
        println!(
            "{name:<14} {} | {} | {}",
            &text[..4],
            &text[4..12],
            &text[12..]
        );
    }

    // Acesso pontual com coordenadas com sinal.
    assert_eq!(
        view.get_bordered(-1, 0, BorderMode::Reflect101).luma(),
        b'b'
    );
    Ok(())
}
