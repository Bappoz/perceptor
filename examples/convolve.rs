//! Correlação e convolução 2D com um kernel arbitrário e `BorderMode`.
//!
//! ```text
//! cargo run --example convolve
//! ```

use perceptor::core::{BorderMode, Result};
use perceptor::imgproc::convolve::{convolve2d, correlate2d, Kernel};
use perceptor::prelude::*;

const SIDE: usize = 8;

/// Degrau vertical: metade esquerda em 50, metade direita em 200.
fn step() -> Result<Image<L8>> {
    Image::from_fn(SIDE, SIDE, |x, _| {
        L8::new(if x < SIDE / 2 { 50 } else { 200 })
    })
}

fn main() -> Result<()> {
    let image = step()?;

    // Blur de caixa 3×3: pesos iguais que somam 1. A saída `L8` é arredondada
    // e saturada; longe do degrau nada muda, sobre ele aparecem os meios-tons.
    let blur = Kernel::new(3, 3, vec![1.0 / 9.0; 9])?;
    let smooth: Image<L8> = correlate2d(image.view(), &blur, BorderMode::Reflect101)?;
    let row: Vec<u8> = smooth.row(0).iter().map(|p| p.luma()).collect();
    assert_eq!(row, [50, 50, 50, 100, 150, 200, 200, 200]);

    // Gradiente horizontal (Sobel): a saída `LF32` guarda o sinal e não satura.
    let sobel_x = Kernel::from_rows([[-1.0, 0.0, 1.0], [-2.0, 0.0, 2.0], [-1.0, 0.0, 1.0]])?;
    let dx: Image<LF32> = correlate2d(image.view(), &sobel_x, BorderMode::Replicate)?;
    assert_eq!(dx.row(0)[3].luma(), 600.0, "4 · (200 − 50) sobre o degrau");
    assert_eq!(dx.row(0)[0].luma(), 0.0, "região plana");

    // A convolução gira o kernel 180°: em um kernel antissimétrico, troca o sinal.
    let flipped: Image<LF32> = convolve2d(image.view(), &sobel_x, BorderMode::Replicate)?;
    assert_eq!(flipped.row(0)[3].luma(), -600.0);

    // A mesma conta com saída `L8` satura: 600 vira 255 e −600 vira 0.
    let clipped: Image<L8> = correlate2d(image.view(), &sobel_x, BorderMode::Replicate)?;
    assert_eq!(clipped.row(0)[3].luma(), 255);

    // A borda decide o que entra na conta na beirada: com zeros em volta, a
    // primeira coluna "vê" um degrau de 0 para 50 que não existe na imagem.
    let zero = BorderMode::Constant(L8::new(0));
    let edge: Image<LF32> = correlate2d(image.view(), &sobel_x, zero)?;
    assert_eq!(edge.row(SIDE / 2)[0].luma(), 200.0, "4 · (50 − 0)");

    println!("blur da linha 0: {row:?}");
    Ok(())
}
