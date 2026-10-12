//! Histogramas por canal, com máscara opcional, e equalização global.

use perceptor_core::{Error, Image, Rgb8, L8};
use perceptor_imgproc::histogram::{
    cumulative, equalization_lut, equalize, histogram, histograms, Histogram,
};
use perceptor_imgproc::lut::apply_lut;
use perceptor_testkit::patterns::noise;
use perceptor_testkit::strategies::any_image;
use proptest::prelude::*;

fn levels(image: &Image<L8>) -> Vec<u8> {
    image.as_pixels().iter().map(|p| p.luma()).collect()
}

/// Imagem `L8` com os níveis dados, em uma linha.
fn row(values: &[u8]) -> Image<L8> {
    Image::from_fn(values.len(), 1, |x, _| L8::new(values[x])).unwrap()
}

fn counts(pairs: &[(usize, u64)]) -> Histogram {
    let mut h = [0u64; 256];
    for &(level, n) in pairs {
        h[level] = n;
    }
    h
}

// ── Histograma ─────────────────────────────────────────────────────────────────

#[test]
fn counts_each_level_by_hand() {
    let image = row(&[0, 0, 7, 255, 7, 7, 128]);
    let h = histogram(image.view(), 0, None).unwrap();
    assert_eq!(h[0], 2);
    assert_eq!(h[7], 3);
    assert_eq!(h[128], 1);
    assert_eq!(h[255], 1);
    assert_eq!(h.iter().sum::<u64>(), 7);
}

#[test]
fn picks_the_requested_channel_of_a_color_image() {
    let image = Image::from_pixels(
        3,
        1,
        &[
            Rgb8::new(1, 10, 100),
            Rgb8::new(1, 20, 100),
            Rgb8::new(2, 20, 100),
        ],
    )
    .unwrap();
    let (r, g, b) = (
        histogram(image.view(), 0, None).unwrap(),
        histogram(image.view(), 1, None).unwrap(),
        histogram(image.view(), 2, None).unwrap(),
    );
    assert_eq!((r[1], r[2]), (2, 1));
    assert_eq!((g[10], g[20]), (1, 2));
    assert_eq!(b[100], 3);
}

#[test]
fn the_mask_selects_which_pixels_count() {
    let image = row(&[5, 5, 9, 9, 9]);
    let mask = row(&[255, 0, 1, 0, 200]);
    let h = histogram(image.view(), 0, Some(mask.view())).unwrap();
    assert_eq!(
        (h[5], h[9]),
        (1, 2),
        "conta onde a máscara é diferente de 0"
    );
    assert_eq!(h.iter().sum::<u64>(), 3);
}

#[test]
fn an_all_zero_mask_gives_an_empty_histogram() {
    let image = noise::<L8>(8, 8, 1);
    let mask = Image::filled(8, 8, L8::new(0)).unwrap();
    let h = histogram(image.view(), 0, Some(mask.view())).unwrap();
    assert!(h.iter().all(|&n| n == 0));
}

#[test]
fn rejects_a_channel_outside_the_format() {
    let image = Image::filled(2, 2, Rgb8::new(1, 2, 3)).unwrap();
    let err = histogram(image.view(), 3, None).unwrap_err();
    assert!(
        matches!(
            err,
            Error::ChannelOutOfRange {
                index: 3,
                channels: 3
            }
        ),
        "{err:?}"
    );
}

#[test]
fn rejects_a_mask_of_another_size() {
    let image = noise::<L8>(8, 6, 1);
    let mask = noise::<L8>(8, 5, 1);
    let err = histogram(image.view(), 0, Some(mask.view())).unwrap_err();
    assert!(
        matches!(
            err,
            Error::ShapeMismatch {
                expected: (8, 6),
                actual: (8, 5)
            }
        ),
        "{err:?}"
    );
}

#[test]
fn histograms_returns_one_per_channel() {
    let image = noise::<Rgb8>(9, 7, 3);
    let all = histograms(image.view(), None).unwrap();
    assert_eq!(all.len(), 3);
    for (channel, h) in all.iter().enumerate() {
        assert_eq!(*h, histogram(image.view(), channel, None).unwrap());
    }
}

#[test]
fn a_strided_roi_counts_only_its_pixels() {
    let image = noise::<L8>(20, 12, 5);
    let region = perceptor_core::Rect::new(4, 3, 7, 5);
    let view = image.roi(region).unwrap();
    let h = histogram(view, 0, None).unwrap();
    assert_eq!(h.iter().sum::<u64>(), 35);
    assert_eq!(
        h,
        histogram(view.to_image().unwrap().view(), 0, None).unwrap()
    );
}

proptest! {
    #[test]
    fn the_histogram_sums_to_the_pixel_count(
        image in any_image::<Rgb8>(1..=24, 1..=24),
        channel in 0usize..3,
    ) {
        let h = histogram(image.view(), channel, None).unwrap();
        prop_assert_eq!(h.iter().sum::<u64>(), (image.width() * image.height()) as u64);
    }

    #[test]
    fn the_masked_histogram_sums_to_the_mask_population(
        image in any_image::<L8>(1..=24, 1..=24),
        seed in any::<u64>(),
    ) {
        let mask = noise::<L8>(image.width(), image.height(), seed);
        let h = histogram(image.view(), 0, Some(mask.view())).unwrap();
        let on = mask.as_pixels().iter().filter(|p| p.luma() != 0).count() as u64;
        prop_assert_eq!(h.iter().sum::<u64>(), on);
    }
}

// ── Cumulativo ─────────────────────────────────────────────────────────────────

#[test]
fn cumulative_is_the_running_sum_ending_at_the_total() {
    let h = counts(&[(0, 2), (3, 5), (255, 1)]);
    let c = cumulative(&h);
    assert_eq!((c[0], c[2], c[3], c[254], c[255]), (2, 2, 7, 7, 8));
    assert!(c.windows(2).all(|w| w[0] <= w[1]));
}

// ── Equalização ────────────────────────────────────────────────────────────────

#[test]
fn equalization_spreads_four_equal_levels_evenly() {
    // cdf = 1, 2, 3, 4; cdf_min = 1; N = 4 → (cdf − 1)/3 · 255 = 0, 85, 170, 255.
    let h = counts(&[(10, 1), (20, 1), (30, 1), (40, 1)]);
    let lut = equalization_lut(&h);
    let s = lut.as_slice();
    assert_eq!((s[10], s[20], s[30], s[40]), (0, 85, 170, 255));
}

#[test]
fn equalization_rounds_halves_up() {
    // cdf = 2, 3, 4; cdf_min = 2; N = 4 → nível 1: (3 − 2)/2 · 255 = 127,5 → 128.
    let h = counts(&[(0, 2), (1, 1), (2, 1)]);
    let s = equalization_lut(&h);
    assert_eq!(&s.as_slice()[..3], &[0, 128, 255]);
}

#[test]
fn levels_between_populated_bins_follow_the_cdf() {
    // Um nível vazio entre dois cheios herda o cdf do anterior.
    let h = counts(&[(10, 1), (50, 3)]);
    let s = equalization_lut(&h);
    assert_eq!(s.as_slice()[10], 0);
    assert_eq!(s.as_slice()[11], 0, "cdf continua em 1 até o nível 49");
    assert_eq!(s.as_slice()[49], 0);
    assert_eq!(s.as_slice()[50], 255);
}

#[test]
fn a_low_contrast_image_fills_the_whole_range() {
    // Níveis só entre 100 e 140.
    let image = Image::from_fn(32, 8, |x, y| {
        L8::new(100 + u8::try_from((x * 5 + y * 3) % 41).unwrap())
    })
    .unwrap();
    let before = levels(&image);
    assert_eq!(*before.iter().min().unwrap(), 100);
    assert_eq!(*before.iter().max().unwrap(), 140);

    let out = levels(&equalize(image.view()).unwrap());

    assert_eq!(*out.iter().min().unwrap(), 0);
    assert_eq!(*out.iter().max().unwrap(), 255);
}

#[test]
fn a_constant_image_is_left_alone() {
    let image = Image::filled(5, 4, L8::new(77)).unwrap();
    let out = equalize(image.view()).unwrap();
    assert_eq!(out.as_pixels(), image.as_pixels());
}

#[test]
fn an_empty_histogram_gives_the_identity() {
    let lut = equalization_lut(&[0u64; 256]);
    assert!(lut
        .as_slice()
        .iter()
        .enumerate()
        .all(|(i, &v)| usize::from(v) == i));
}

#[test]
fn equalize_is_the_lut_of_its_own_histogram() {
    let image = noise::<L8>(24, 16, 11);
    let h = histogram(image.view(), 0, None).unwrap();
    let expect = apply_lut(image.view(), &equalization_lut(&h)).unwrap();
    assert_eq!(
        equalize(image.view()).unwrap().as_pixels(),
        expect.as_pixels()
    );
}

proptest! {
    #[test]
    fn the_equalization_lut_is_monotonic_and_spans_the_range(
        values in prop::collection::vec(0u8..=255, 2..64),
    ) {
        let image = row(&values);
        let h = histogram(image.view(), 0, None).unwrap();
        let lut = equalization_lut(&h);
        let s = lut.as_slice();

        prop_assert!(s.windows(2).all(|w| w[0] <= w[1]));
        let (lo, hi) = (*values.iter().min().unwrap(), *values.iter().max().unwrap());
        if lo != hi {
            prop_assert_eq!(s[usize::from(lo)], 0);
            prop_assert_eq!(s[usize::from(hi)], 255);
        }
    }

    #[test]
    fn equalization_preserves_the_order_of_pixels(image in any_image::<L8>(1..=16, 1..=16)) {
        let out = equalize(image.view()).unwrap();
        for (a, b) in image.as_pixels().iter().zip(out.as_pixels()) {
            for (c, d) in image.as_pixels().iter().zip(out.as_pixels()) {
                if a.luma() <= c.luma() {
                    prop_assert!(b.luma() <= d.luma());
                }
            }
        }
    }
}
