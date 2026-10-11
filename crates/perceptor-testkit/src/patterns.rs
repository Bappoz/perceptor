//! Imagens sintéticas determinísticas.
//!
//! Todas entram em pânico com dimensões inválidas: são helpers de teste, e
//! uma dimensão inválida é erro do teste.

use perceptor_core::{Image, Pixel};

use crate::sample::from_unit;

/// Pixel com todos os canais em `value`.
fn splat<P: Pixel>(value: P::Subpixel) -> P {
    let mut pixel = P::default();
    pixel.channels_mut().fill(value);
    pixel
}

/// Imagem `width × height` com `f(x, y)` como fração do intervalo nominal em todos os canais.
fn from_unit_fn<P: Pixel>(
    width: usize,
    height: usize,
    f: impl Fn(usize, usize) -> f64,
) -> Image<P> {
    Image::from_fn(width, height, |x, y| splat(from_unit(f(x, y))))
        .expect("dimensões do padrão sintético devem ser válidas")
}

/// Rampa horizontal de `0` ao fundo de escala; todos os canais iguais.
///
/// Com largura 1 a imagem inteira é `0`.
///
/// # Panics
/// Se alguma dimensão for zero.
#[must_use]
pub fn gradient<P: Pixel>(width: usize, height: usize) -> Image<P> {
    #[allow(clippy::cast_precision_loss)] // dimensões de teste, muito abaixo de 2^52
    from_unit_fn(width, height, |x, _| {
        if width > 1 {
            x as f64 / (width - 1) as f64
        } else {
            0.0
        }
    })
}

/// Xadrez de células `cell × cell`: fundo de escala onde `x/cell + y/cell` é par, zero nas demais.
///
/// # Panics
/// Se alguma dimensão ou `cell` for zero.
#[must_use]
pub fn checkerboard<P: Pixel>(width: usize, height: usize, cell: usize) -> Image<P> {
    assert!(cell > 0, "a célula do xadrez precisa de pelo menos 1 pixel");
    from_unit_fn(width, height, |x, y| {
        if (x / cell + y / cell).is_multiple_of(2) {
            1.0
        } else {
            0.0
        }
    })
}

/// Imagem zerada com fundo de escala (em todos os canais) apenas em `(x, y)`.
///
/// # Panics
/// Se alguma dimensão for zero ou `(x, y)` estiver fora da imagem.
#[must_use]
pub fn impulse<P: Pixel>(width: usize, height: usize, x: usize, y: usize) -> Image<P> {
    assert!(x < width && y < height, "impulso fora da imagem");
    from_unit_fn(width, height, |px, py| {
        f64::from(u8::from((px, py) == (x, y)))
    })
}

/// Ruído uniforme reproduzível: a mesma `seed` gera a mesma imagem em qualquer máquina.
///
/// # Panics
/// Se alguma dimensão for zero.
#[must_use]
pub fn noise<P: Pixel>(width: usize, height: usize, seed: u64) -> Image<P> {
    // xorshift64*: sem dependência, e a sequência é a mesma em qualquer plataforma.
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut next = move || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    };
    let mut image =
        Image::<P>::new(width, height).expect("dimensões do padrão sintético devem ser válidas");
    for sample in image.as_subpixels_mut() {
        // 53 bits altos → fração uniforme em [0, 1).
        #[allow(clippy::cast_precision_loss)]
        let t = (next() >> 11) as f64 / (1u64 << 53) as f64;
        *sample = from_unit(t);
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    use perceptor_core::{Rgb8, L8, LF32};

    #[test]
    fn gradient_ramps_across_the_width() {
        let img = gradient::<L8>(4, 2);
        assert_eq!(img.row(0), [0, 85, 170, 255].map(L8::new));
        assert_eq!(img.row(1), img.row(0));
        assert_eq!(gradient::<L8>(1, 3).as_pixels(), [L8::new(0); 3]);
    }

    #[test]
    fn gradient_sets_every_channel() {
        let img = gradient::<Rgb8>(2, 1);
        assert_eq!(img[(1, 0)], Rgb8::new(255, 255, 255));
    }

    #[test]
    fn checkerboard_alternates_by_cell() {
        let img = checkerboard::<L8>(4, 4, 2);
        assert_eq!(img[(0, 0)], L8::new(255));
        assert_eq!(img[(1, 1)], L8::new(255));
        assert_eq!(img[(2, 0)], L8::new(0));
        assert_eq!(img[(0, 2)], L8::new(0));
        assert_eq!(img[(2, 2)], L8::new(255));
    }

    #[test]
    fn impulse_has_a_single_nonzero_pixel() {
        let img = impulse::<Rgb8>(5, 4, 3, 1);
        let lit: Vec<_> = (0..4)
            .flat_map(|y| (0..5).map(move |x| (x, y)))
            .filter(|&(x, y)| img[(x, y)] != Rgb8::default())
            .collect();
        assert_eq!(lit, [(3, 1)]);
        assert_eq!(img[(3, 1)], Rgb8::new(255, 255, 255));
    }

    #[test]
    fn noise_is_reproducible_per_seed() {
        assert_eq!(noise::<Rgb8>(8, 8, 1), noise::<Rgb8>(8, 8, 1));
        assert_ne!(noise::<Rgb8>(8, 8, 1), noise::<Rgb8>(8, 8, 2));
    }

    #[test]
    fn noise_stays_in_the_nominal_float_range() {
        let img = noise::<LF32>(16, 16, 7);
        assert!(img
            .as_pixels()
            .iter()
            .all(|p| (0.0..=1.0).contains(&p.luma())));
        let distinct = img.as_pixels().iter().filter(|p| p.luma() != 0.0).count();
        assert!(distinct > 200);
    }
}
