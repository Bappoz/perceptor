//! Estratégias `proptest` de imagens.

use std::ops::RangeInclusive;

use perceptor_core::{Image, Pixel};
use proptest::collection::vec;
use proptest::prelude::*;

use crate::sample::from_unit;

/// Imagens com largura e altura nas faixas dadas e amostras uniformes em todo o intervalo nominal.
///
/// Encolhe em direção a imagens pequenas e de amostras zero.
///
/// # Panics
/// Ao gerar um valor, se uma das faixas incluir `0`.
pub fn any_image<P: Pixel>(
    width: RangeInclusive<usize>,
    height: RangeInclusive<usize>,
) -> impl Strategy<Value = Image<P>> {
    (width, height).prop_flat_map(|(w, h)| {
        vec(0.0..=1.0_f64, w * h * P::FORMAT.channels()).prop_map(move |samples| {
            let mut image = Image::<P>::new(w, h).expect("dimensões geradas devem ser válidas");
            for (sample, t) in image.as_subpixels_mut().iter_mut().zip(samples) {
                *sample = from_unit(t);
            }
            image
        })
    })
}

/// [`any_image`] com dimensões de `1` a `16`: o padrão para testes de propriedade rápidos.
pub fn small_image<P: Pixel>() -> impl Strategy<Value = Image<P>> {
    any_image(1..=16, 1..=16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use perceptor_core::{Rgb8, L16, LF32};

    proptest! {
        #[test]
        fn dimensions_stay_in_range(img in any_image::<Rgb8>(2..=5, 3..=4)) {
            prop_assert!((2..=5).contains(&img.width()));
            prop_assert!((3..=4).contains(&img.height()));
        }

        #[test]
        fn float_samples_stay_in_the_unit_range(img in small_image::<LF32>()) {
            prop_assert!(img.as_pixels().iter().all(|p| (0.0..=1.0).contains(&p.luma())));
        }
    }

    #[test]
    fn strategy_reaches_both_ends_of_the_range() {
        use proptest::strategy::ValueTree;
        use proptest::test_runner::TestRunner;

        let mut runner = TestRunner::deterministic();
        let strategy = any_image::<L16>(8..=8, 8..=8);
        let (mut low, mut high) = (u16::MAX, u16::MIN);
        for _ in 0..64 {
            let img = strategy.new_tree(&mut runner).unwrap().current();
            for p in img.as_pixels() {
                low = low.min(p.luma());
                high = high.max(p.luma());
            }
        }
        assert!(low < 1000 && high > 64000, "{low}..{high}");
    }
}
