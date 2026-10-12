//! Limiarização global: tipos, variante no lugar e Otsu.

use perceptor_core::{Image, Rect, L8};
use perceptor_imgproc::threshold::{
    otsu_from_histogram, otsu_level, threshold, threshold_in_place, threshold_otsu, ThresholdType,
};
use perceptor_testkit::assert_images_eq;
use perceptor_testkit::patterns::noise;
use perceptor_testkit::strategies::any_image;
use proptest::prelude::*;

const TYPES: [ThresholdType; 5] = [
    ThresholdType::Binary,
    ThresholdType::BinaryInv,
    ThresholdType::Trunc,
    ThresholdType::ToZero,
    ThresholdType::ToZeroInv,
];

fn kind() -> impl Strategy<Value = ThresholdType> {
    prop::sample::select(TYPES.to_vec())
}

/// Todos os 256 níveis, um por pixel.
fn ramp() -> Image<L8> {
    Image::from_fn(256, 1, |x, _| L8::new(u8::try_from(x).unwrap())).unwrap()
}

fn levels(image: &Image<L8>) -> Vec<u8> {
    image.as_pixels().iter().map(|p| p.luma()).collect()
}

/// Imagem com `low_count` pixels em `low` e o resto em `high`.
fn two_levels(low: u8, high: u8, low_count: usize, total: usize) -> Image<L8> {
    Image::from_fn(total, 1, |x, _| {
        L8::new(if x < low_count { low } else { high })
    })
    .unwrap()
}

// ── Tipos ──────────────────────────────────────────────────────────────────────

#[test]
fn each_type_follows_its_definition_on_every_level() {
    let (t, max) = (100u8, 200u8);
    let input = ramp();
    let expect = |kind: ThresholdType, v: u8| match kind {
        ThresholdType::Binary => {
            if v > t {
                max
            } else {
                0
            }
        }
        ThresholdType::BinaryInv => {
            if v > t {
                0
            } else {
                max
            }
        }
        ThresholdType::Trunc => {
            if v > t {
                t
            } else {
                v
            }
        }
        ThresholdType::ToZero => {
            if v > t {
                v
            } else {
                0
            }
        }
        ThresholdType::ToZeroInv => {
            if v > t {
                0
            } else {
                v
            }
        }
    };
    for kind in TYPES {
        let output = threshold(input.view(), t, max, kind).unwrap();
        let expected: Vec<u8> = (0..=255u8).map(|v| expect(kind, v)).collect();
        assert_eq!(levels(&output), expected, "{kind:?}");
    }
}

#[test]
fn the_comparison_is_strictly_greater_than() {
    let input = Image::from_pixels(3, 1, &[L8::new(99), L8::new(100), L8::new(101)]).unwrap();
    let output = threshold(input.view(), 100, 255, ThresholdType::Binary).unwrap();
    assert_eq!(levels(&output), [0, 0, 255]);
}

#[test]
fn extreme_thresholds() {
    let input = ramp();
    // Nada é maior que 255.
    let none = threshold(input.view(), 255, 255, ThresholdType::Binary).unwrap();
    assert!(levels(&none).iter().all(|&v| v == 0));
    // Tudo menos o 0 é maior que 0.
    let all = threshold(input.view(), 0, 255, ThresholdType::Binary).unwrap();
    assert_eq!(all.row(0)[0], L8::new(0));
    assert!(levels(&all)[1..].iter().all(|&v| v == 255));
}

#[test]
fn default_type_is_binary() {
    assert_eq!(ThresholdType::default(), ThresholdType::Binary);
}

proptest! {
    #[test]
    fn binary_output_only_contains_zero_and_max(
        image in any_image::<L8>(1..=24, 1..=24),
        t in any::<u8>(),
        max in any::<u8>(),
    ) {
        for kind in [ThresholdType::Binary, ThresholdType::BinaryInv] {
            let output = threshold(image.view(), t, max, kind).unwrap();
            prop_assert!(levels(&output).iter().all(|&v| v == 0 || v == max), "{kind:?}");
        }
    }

    #[test]
    fn binary_and_inverse_are_complementary(
        image in any_image::<L8>(1..=24, 1..=24),
        t in any::<u8>(),
        max in any::<u8>(),
    ) {
        let direct = threshold(image.view(), t, max, ThresholdType::Binary).unwrap();
        let inverse = threshold(image.view(), t, max, ThresholdType::BinaryInv).unwrap();
        for (a, b) in levels(&direct).into_iter().zip(levels(&inverse)) {
            prop_assert_eq!(u16::from(a) + u16::from(b), u16::from(max));
        }
    }

    #[test]
    fn the_other_types_never_invent_levels(
        image in any_image::<L8>(1..=24, 1..=24),
        t in any::<u8>(),
    ) {
        let input = levels(&image);
        let trunc = threshold(image.view(), t, 255, ThresholdType::Trunc).unwrap();
        prop_assert!(levels(&trunc).iter().zip(&input).all(|(&o, &i)| o == i.min(t)));
        // `ToZero` e `ToZeroInv` repartem a imagem: somados, devolvem a original.
        let keep = threshold(image.view(), t, 255, ThresholdType::ToZero).unwrap();
        let drop = threshold(image.view(), t, 255, ThresholdType::ToZeroInv).unwrap();
        for ((k, d), i) in levels(&keep).into_iter().zip(levels(&drop)).zip(&input) {
            prop_assert!((k == *i && d == 0) || (k == 0 && d == *i));
        }
    }

    #[test]
    fn in_place_matches_the_allocating_version(
        image in any_image::<L8>(1..=24, 1..=24),
        t in any::<u8>(),
        max in any::<u8>(),
        kind in kind(),
    ) {
        let expected = threshold(image.view(), t, max, kind).unwrap();
        let mut target = image.clone();
        threshold_in_place(target.view_mut(), t, max, kind);
        assert_images_eq!(target, expected);
    }

    #[test]
    fn a_roi_is_thresholded_like_the_same_region_of_the_whole(
        image in any_image::<L8>(4..=16, 4..=16),
        t in any::<u8>(),
        kind in kind(),
    ) {
        let rect = Rect::new(1, 1, image.width() - 2, image.height() - 2);
        let whole = threshold(image.view(), t, 255, kind).unwrap();
        let part = threshold(image.roi(rect).unwrap(), t, 255, kind).unwrap();
        for (got, want) in part.rows().zip(whole.roi(rect).unwrap().rows()) {
            prop_assert_eq!(got, want);
        }
    }
}

// ── Otsu ───────────────────────────────────────────────────────────────────────

#[test]
fn otsu_falls_between_the_two_modes_of_a_bimodal_image() {
    // Dois modos com espalhamento: 60 ± 20 e 190 ± 20, em proporção 2:1.
    let spread = noise::<L8>(300, 100, 5);
    let image = Image::from_fn(300, 100, |x, y| {
        let jitter = i32::from(spread.row(y)[x].luma()) * 40 / 255 - 20;
        let mode = if x < 200 { 60 } else { 190 };
        L8::new(u8::try_from(mode + jitter).unwrap())
    })
    .unwrap();

    let level = otsu_level(image.view());

    assert!((80..170).contains(&level), "limiar {level}");
    // E separa os modos por inteiro.
    let (used, binary) = threshold_otsu(image.view(), 255, ThresholdType::Binary).unwrap();
    assert_eq!(used, level);
    for row in binary.rows() {
        assert!(row[..200].iter().all(|p| p.luma() == 0));
        assert!(row[200..].iter().all(|p| p.luma() == 255));
    }
}

#[test]
fn otsu_of_two_spikes_is_the_middle_of_the_gap() {
    let image = two_levels(50, 200, 30, 100);
    // Qualquer limiar em 50..200 separa igualmente bem; o meio é 124.
    assert_eq!(otsu_level(image.view()), 124);
}

#[test]
fn otsu_of_a_uniform_image_is_its_only_level() {
    let image = Image::filled(8, 8, L8::new(77)).unwrap();
    assert_eq!(otsu_level(image.view()), 77);
    // Nada é maior que o próprio nível: a saída binária é toda zero.
    let (_, binary) = threshold_otsu(image.view(), 255, ThresholdType::Binary).unwrap();
    assert!(levels(&binary).iter().all(|&v| v == 0));
}

#[test]
fn otsu_from_histogram_matches_the_image() {
    let image = noise::<L8>(64, 48, 9);
    let mut histogram = [0u64; 256];
    for pixel in image.as_pixels() {
        histogram[usize::from(pixel.luma())] += 1;
    }
    assert_eq!(otsu_from_histogram(&histogram), otsu_level(image.view()));
    assert_eq!(otsu_from_histogram(&[0; 256]), 0, "histograma vazio");
}

proptest! {
    #[test]
    fn otsu_separates_any_two_levels(
        low in 0u8..255,
        gap in 1u8..=255,
        low_count in 1usize..40,
        high_count in 1usize..40,
    ) {
        let high = low.saturating_add(gap).max(low + 1);
        let image = two_levels(low, high, low_count, low_count + high_count);

        let level = otsu_level(image.view());

        prop_assert!(low <= level && level < high, "{low} ≤ {level} < {high}");
    }

    #[test]
    fn otsu_does_not_depend_on_pixel_order(image in any_image::<L8>(2..=16, 2..=16)) {
        let mut sorted = levels(&image);
        sorted.sort_unstable();
        let reordered = Image::from_subpixels(image.width(), image.height(), &sorted).unwrap();
        prop_assert_eq!(otsu_level(reordered.view()), otsu_level(image.view()));
    }
}
