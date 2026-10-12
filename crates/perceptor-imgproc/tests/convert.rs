//! Conversão de profundidade (u8 ↔ u16 ↔ f32) e normalização.

// Os esperados exatos (0,0, 0,25, 1,0…) são representáveis em binário, e os
// casts para `f32` são de médias/desvios de imagens de poucos pixels.
#![allow(
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]

use perceptor_core::{Error, Image, Rect, Rgb8, RgbF32, L16, L8, LF32};
use perceptor_imgproc::convert::{
    channel_mean_std, convert_depth, convert_scale, normalize_mean_std, normalize_minmax,
};
use perceptor_testkit::patterns::noise;
use perceptor_testkit::strategies::any_image;
use proptest::prelude::*;

fn l8(values: &[u8]) -> Image<L8> {
    Image::from_fn(values.len(), 1, |x, _| L8::new(values[x])).unwrap()
}

fn l8_values(image: &Image<L8>) -> Vec<u8> {
    image.as_pixels().iter().map(|p| p.luma()).collect()
}

fn lf32_values(image: &Image<LF32>) -> Vec<f32> {
    image.as_pixels().iter().map(|p| p.luma()).collect()
}

fn lf32(values: &[f32]) -> Image<LF32> {
    Image::from_fn(values.len(), 1, |x, _| LF32::new(values[x])).unwrap()
}

/// Todos os 256 níveis de 8 bits, um por pixel.
fn all_levels() -> Image<L8> {
    l8(&(0..=255).collect::<Vec<u8>>())
}

// ── convert_scale ──────────────────────────────────────────────────────────────

#[test]
fn convert_scale_matches_hand_computed_values() {
    // out = round(2·v − 50), saturado em [0, 255].
    let image = l8(&[0, 10, 25, 26, 100, 177, 178, 200, 255]);
    let out: Image<L8> = convert_scale(image.view(), 2.0, -50.0).unwrap();
    assert_eq!(l8_values(&out), [0, 0, 0, 2, 150, 255, 255, 255, 255]);
}

#[test]
fn convert_scale_rounds_halves_away_from_zero() {
    let out: Image<L8> = convert_scale(l8(&[1, 3, 5]).view(), 0.5, 0.0).unwrap();
    assert_eq!(l8_values(&out), [1, 2, 3], "0,5 → 1; 1,5 → 2; 2,5 → 3");
}

#[test]
fn convert_scale_saturates_at_both_ends_for_every_target_depth() {
    let image = l8(&[0, 128, 255]);
    let up: Image<L8> = convert_scale(image.view(), 10.0, 0.0).unwrap();
    assert_eq!(l8_values(&up), [0, 255, 255]);
    let down: Image<L8> = convert_scale(image.view(), 1.0, -300.0).unwrap();
    assert_eq!(l8_values(&down), [0, 0, 0]);
    let wide: Image<L16> = convert_scale(image.view(), 1000.0, 0.0).unwrap();
    assert_eq!(
        wide.as_pixels()
            .iter()
            .map(|p| p.luma())
            .collect::<Vec<_>>(),
        [0, 65_535, 65_535]
    );
}

#[test]
fn convert_scale_to_f32_does_not_clamp() {
    let out: Image<LF32> = convert_scale(l8(&[0, 255]).view(), 0.5, -100.0).unwrap();
    assert_eq!(lf32_values(&out), [-100.0, 27.5]);
}

proptest! {
    #[test]
    fn convert_scale_never_overflows_and_follows_the_formula(
        image in any_image::<L8>(1..=12, 1..=12),
        alpha in -1000.0f64..1000.0,
        beta in -100_000.0f64..100_000.0,
    ) {
        let out: Image<L8> = convert_scale(image.view(), alpha, beta).unwrap();
        for (src, dst) in image.as_pixels().iter().zip(out.as_pixels()) {
            let expect = (alpha * f64::from(src.luma()) + beta).round().clamp(0.0, 255.0);
            prop_assert!((f64::from(dst.luma()) - expect).abs() < 0.5);
        }
    }

    #[test]
    fn convert_scale_saturates_f32_inputs_with_any_magnitude(
        values in prop::collection::vec(-1.0e30f32..1.0e30, 1..16),
    ) {
        let out: Image<L8> = convert_scale(lf32(&values).view(), 1.0, 0.0).unwrap();
        for (v, o) in values.iter().zip(out.as_pixels()) {
            let expect = f64::from(*v).round().clamp(0.0, 255.0);
            prop_assert!((f64::from(o.luma()) - expect).abs() < 0.5, "{v}");
        }
    }
}

#[test]
fn convert_scale_maps_nan_to_zero() {
    let out: Image<L8> = convert_scale(lf32(&[f32::NAN]).view(), 1.0, 0.0).unwrap();
    assert_eq!(l8_values(&out), [0]);
}

// ── convert_depth ──────────────────────────────────────────────────────────────

#[test]
fn u8_to_f32_and_back_is_exact_for_every_level() {
    let image = all_levels();
    let wide: Image<LF32> = convert_depth(image.view()).unwrap();
    let back: Image<L8> = convert_depth(wide.view()).unwrap();
    assert_eq!(l8_values(&back), l8_values(&image));
    assert_eq!(lf32_values(&wide)[0], 0.0);
    assert_eq!(lf32_values(&wide)[255], 1.0);
}

#[test]
fn u8_to_u16_and_back_is_exact_and_uses_the_full_range() {
    let image = all_levels();
    let wide: Image<L16> = convert_depth(image.view()).unwrap();
    let values: Vec<u16> = wide.as_pixels().iter().map(|p| p.luma()).collect();
    assert_eq!(
        (values[0], values[1], values[128], values[255]),
        (0, 257, 32_896, 65_535)
    );
    let back: Image<L8> = convert_depth(wide.view()).unwrap();
    assert_eq!(l8_values(&back), l8_values(&image));
}

#[test]
fn u16_to_u8_rounds_to_nearest_at_the_boundary() {
    // 32767 / 257 = 127,498 → 127;  32768 / 257 = 127,502 → 128.
    let image = Image::from_fn(2, 1, |x, _| L16::new(32_767 + u16::try_from(x).unwrap())).unwrap();
    let out: Image<L8> = convert_depth(image.view()).unwrap();
    assert_eq!(l8_values(&out), [127, 128]);
}

#[test]
fn f32_outside_the_nominal_range_saturates() {
    let image = lf32(&[-0.5, 0.0, 0.5, 1.0, 1.5]);
    let out: Image<L8> = convert_depth(image.view()).unwrap();
    // 0,5 · 255 = 127,5 → 128.
    assert_eq!(l8_values(&out), [0, 0, 128, 255, 255]);
}

proptest! {
    #[test]
    fn rgb_round_trip_through_f32_is_exact(image in any_image::<Rgb8>(1..=16, 1..=16)) {
        let wide: Image<RgbF32> = convert_depth(image.view()).unwrap();
        let back: Image<Rgb8> = convert_depth(wide.view()).unwrap();
        prop_assert_eq!(back.as_pixels(), image.as_pixels());
    }

    #[test]
    fn u16_round_trip_through_f32_loses_at_most_one_level(
        values in prop::collection::vec(any::<u16>(), 1..32),
    ) {
        let image = Image::from_fn(values.len(), 1, |x, _| L16::new(values[x])).unwrap();
        let wide: Image<LF32> = convert_depth(image.view()).unwrap();
        let back: Image<L16> = convert_depth(wide.view()).unwrap();
        for (a, b) in image.as_pixels().iter().zip(back.as_pixels()) {
            prop_assert!(a.luma().abs_diff(b.luma()) <= 1);
        }
    }
}

#[test]
fn a_channel_count_mismatch_is_an_error() {
    let image = Image::filled(2, 2, Rgb8::new(1, 2, 3)).unwrap();
    let err = convert_depth::<Rgb8, LF32>(image.view()).unwrap_err();
    assert!(
        matches!(
            err,
            Error::ChannelMismatch {
                expected: 1,
                actual: 3
            }
        ),
        "{err:?}"
    );
    let err = convert_scale::<Rgb8, LF32>(image.view(), 1.0, 0.0).unwrap_err();
    assert!(matches!(err, Error::ChannelMismatch { .. }), "{err:?}");
}

#[test]
fn a_strided_roi_converts_like_its_copy() {
    let image = noise::<L8>(20, 12, 5);
    let view = image.roi(Rect::new(3, 2, 9, 7)).unwrap();
    let from_view: Image<LF32> = convert_depth(view).unwrap();
    let from_copy: Image<LF32> = convert_depth(view.to_image().unwrap().view()).unwrap();
    assert_eq!(from_view.as_pixels(), from_copy.as_pixels());
}

// ── Estatísticas e normalização ────────────────────────────────────────────────

#[test]
fn channel_mean_std_matches_hand_computed_values() {
    // {0, 255} em tons nominais {0, 1}: média 0,5 e desvio populacional 0,5.
    let stats = channel_mean_std(l8(&[0, 255]).view());
    assert_eq!(stats.len(), 1);
    assert!((stats[0].mean - 0.5).abs() < 1e-12);
    assert!((stats[0].std - 0.5).abs() < 1e-12);
}

#[test]
fn channel_mean_std_is_per_channel() {
    // R constante em 51 (0,2); G em {0, 255}; B constante em 255.
    let image =
        Image::from_pixels(2, 1, &[Rgb8::new(51, 0, 255), Rgb8::new(51, 255, 255)]).unwrap();
    let stats = channel_mean_std(image.view());
    assert!((stats[0].mean - 0.2).abs() < 1e-12 && stats[0].std.abs() < 1e-12);
    assert!((stats[1].mean - 0.5).abs() < 1e-12 && (stats[1].std - 0.5).abs() < 1e-12);
    assert!((stats[2].mean - 1.0).abs() < 1e-12 && stats[2].std.abs() < 1e-12);
}

#[test]
fn minmax_stretches_the_observed_range_onto_the_target() {
    let image = l8(&[10, 20, 30]);
    let unit: Image<LF32> = normalize_minmax(image.view(), 0.0, 1.0).unwrap();
    assert_eq!(lf32_values(&unit), [0.0, 0.5, 1.0]);
    let signed: Image<LF32> = normalize_minmax(image.view(), -1.0, 1.0).unwrap();
    assert_eq!(lf32_values(&signed), [-1.0, 0.0, 1.0]);
}

#[test]
fn minmax_uses_the_range_of_all_channels_together() {
    // R em {0, 100}, G em {50, 200}: mínimo global 0, máximo global 200.
    let image = Image::from_pixels(2, 1, &[Rgb8::new(0, 50, 0), Rgb8::new(100, 200, 0)]).unwrap();
    let out: Image<RgbF32> = normalize_minmax(image.view(), 0.0, 1.0).unwrap();
    let p = out.as_pixels();
    assert_eq!((p[0].0[0], p[0].0[1]), (0.0, 0.25));
    assert_eq!((p[1].0[0], p[1].0[1]), (0.5, 1.0));
}

#[test]
fn minmax_of_a_constant_image_is_the_lower_target() {
    let image = Image::filled(3, 3, L8::new(77)).unwrap();
    let out: Image<LF32> = normalize_minmax(image.view(), -2.0, 5.0).unwrap();
    assert!(lf32_values(&out).iter().all(|&v| v == -2.0));
}

#[test]
fn mean_std_applies_the_standard_formula_per_channel() {
    // (v − mean) / std em tons nominais: {0, 255} com média 0,5, desvio 0,5.
    let out: Image<LF32> = normalize_mean_std(l8(&[0, 255]).view(), &[0.5], &[0.5]).unwrap();
    assert_eq!(lf32_values(&out), [-1.0, 1.0]);

    let image = Image::filled(1, 1, Rgb8::new(255, 0, 51)).unwrap();
    let out: Image<RgbF32> =
        normalize_mean_std(image.view(), &[0.5, 0.25, 0.2], &[0.5, 0.25, 0.1]).unwrap();
    let p = out.as_pixels()[0].0;
    assert!((p[0] - 1.0).abs() < 1e-6 && (p[1] + 1.0).abs() < 1e-6 && p[2].abs() < 1e-6);
}

#[test]
fn mean_std_rejects_a_wrong_number_of_parameters() {
    let image = Image::filled(2, 2, Rgb8::new(1, 2, 3)).unwrap();
    let err =
        normalize_mean_std::<Rgb8, RgbF32>(image.view(), &[0.5], &[0.5, 0.5, 0.5]).unwrap_err();
    assert!(
        matches!(
            err,
            Error::ChannelMismatch {
                expected: 3,
                actual: 1
            }
        ),
        "{err:?}"
    );
}

#[test]
fn mean_std_rejects_a_non_positive_or_non_finite_std() {
    let image = l8(&[1, 2]);
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let err = normalize_mean_std::<L8, LF32>(image.view(), &[0.5], &[bad]).unwrap_err();
        assert!(
            matches!(err, Error::InvalidParameter { name: "std", .. }),
            "{bad}: {err:?}"
        );
    }
}

proptest! {
    #[test]
    fn standardizing_by_its_own_statistics_gives_zero_mean_and_unit_std(
        image in any_image::<Rgb8>(2..=16, 2..=16),
    ) {
        let stats = channel_mean_std(image.view());
        // Canal constante não tem desvio a normalizar.
        prop_assume!(stats.iter().all(|s| s.std > 1e-3));
        let mean: Vec<f32> = stats.iter().map(|s| s.mean as f32).collect();
        let std: Vec<f32> = stats.iter().map(|s| s.std as f32).collect();

        let out: Image<RgbF32> = normalize_mean_std(image.view(), &mean, &std).unwrap();

        let rounded = channel_mean_std_f32(&out);
        for (m, s) in rounded {
            prop_assert!(m.abs() < 1e-3, "média {m}");
            prop_assert!((s - 1.0).abs() < 1e-3, "desvio {s}");
        }
    }
}

/// Média e desvio populacional de cada canal de uma imagem `f32`.
fn channel_mean_std_f32(image: &Image<RgbF32>) -> Vec<(f64, f64)> {
    let n = image.as_pixels().len() as f64;
    (0..3)
        .map(|c| {
            let values: Vec<f64> = image
                .as_pixels()
                .iter()
                .map(|p| f64::from(p.0[c]))
                .collect();
            let mean = values.iter().sum::<f64>() / n;
            let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
            (mean, var.sqrt())
        })
        .collect()
}
