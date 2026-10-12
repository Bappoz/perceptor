//! CLAHE: equalização adaptativa com limite de contraste.
//!
//! ```text
//! cargo run --example clahe
//! ```

use perceptor::core::Result;
use perceptor::imgproc::clahe::clahe;
use perceptor::imgproc::histogram::equalize;
use perceptor::prelude::*;

const WIDTH: usize = 128;

/// Rampa suave de 0 a 254: sem estrutura local alguma.
fn ramp() -> Result<Image<L8>> {
    Image::from_fn(WIDTH, 32, |x, _| {
        L8::new(u8::try_from(x * 254 / (WIDTH - 1)).unwrap_or(254))
    })
}

/// Maior diferença entre dois pixels vizinhos de uma linha.
fn max_jump(image: &Image<L8>) -> u8 {
    image
        .as_pixels()
        .chunks_exact(image.width())
        .flat_map(|row| row.windows(2).map(|w| w[0].luma().abs_diff(w[1].luma())))
        .max()
        .unwrap_or(0)
}

fn main() -> Result<()> {
    let image = ramp()?;
    println!("rampa:               salto máximo {}", max_jump(&image));

    // Grade 4×1, limite 2: cada tile ganha a própria curva, e as curvas
    // são interpoladas entre os centros dos tiles.
    let adaptive = clahe(image.view(), 2.0, (4, 1))?;
    println!("CLAHE 4×1, limite 2: salto máximo {}", max_jump(&adaptive));
    assert!(max_jump(&adaptive) <= 12);

    // Uma grade 1×1 sem corte é a equalização global (a menos do `cdf_min`).
    let global = equalize(image.view())?;
    println!("equalização global:  salto máximo {}", max_jump(&global));

    // Parâmetro inválido é erro tipado.
    assert!(clahe(image.view(), 2.0, (0, 1)).is_err());
    assert!(clahe(image.view(), -1.0, (4, 1)).is_err());
    Ok(())
}
