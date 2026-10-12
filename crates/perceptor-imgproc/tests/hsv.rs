//! Conversões RGB ↔ HSV/HSL: valores canônicos e round-trips.

use perceptor_core::{Hsl8, HslF32, Hsv8, HsvF32, Image, Pixel, Rect, Rgb8, RgbF32};
use perceptor_imgproc::hsv::{
    hsl_to_rgb, hsv_to_rgb, rgb_to_hsl, rgb_to_hsv, F32_ROUND_TRIP_TOLERANCE,
    HSL8_ROUND_TRIP_TOLERANCE, HSV8_ROUND_TRIP_TOLERANCE,
};
use perceptor_testkit::assert_images_eq;
use perceptor_testkit::strategies::any_image;
use proptest::prelude::*;

fn one<A: Pixel, B: Pixel>(
    pixel: A,
    f: impl Fn(perceptor_core::ImageView<'_, A>) -> perceptor_core::Result<Image<B>>,
) -> B {
    f(Image::filled(1, 1, pixel).unwrap().view())
        .unwrap()
        .as_pixels()[0]
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-5
}

// ── Valores canônicos, u8 ──────────────────────────────────────────────────────

/// `(RGB, matiz em graus)` das primárias e secundárias.
const HUES: [((u8, u8, u8), u16); 6] = [
    ((255, 0, 0), 0),
    ((255, 255, 0), 60),
    ((0, 255, 0), 120),
    ((0, 255, 255), 180),
    ((0, 0, 255), 240),
    ((255, 0, 255), 300),
];

#[test]
fn primaries_and_secondaries_in_hsv8() {
    for ((r, g, b), degrees) in HUES {
        let hsv = one(Rgb8::new(r, g, b), rgb_to_hsv);
        let half = u8::try_from(degrees / 2).unwrap();
        assert_eq!(hsv, Hsv8::new(half, 255, 255), "rgb({r}, {g}, {b})");
        assert_eq!(one(hsv, hsv_to_rgb), Rgb8::new(r, g, b));
    }
}

#[test]
fn primaries_and_secondaries_in_hsl8() {
    for ((r, g, b), degrees) in HUES {
        let hsl = one(Rgb8::new(r, g, b), rgb_to_hsl);
        let half = u8::try_from(degrees / 2).unwrap();
        // L = 0.5 → 127.5, arredondado para 128.
        assert_eq!(hsl, Hsl8::new(half, 255, 128), "rgb({r}, {g}, {b})");
        // 128 não é exatamente a metade de 255: a volta erra um nível nos
        // canais que eram 0.
        let back = one(hsl, hsl_to_rgb);
        let lift = |v: u8| if v == 0 { 1 } else { v };
        assert_eq!(back, Rgb8::new(lift(r), lift(g), lift(b)));
    }
}

#[test]
fn grays_have_zero_hue_and_saturation() {
    for v in [0u8, 1, 64, 128, 200, 255] {
        let gray = Rgb8::new(v, v, v);
        assert_eq!(one(gray, rgb_to_hsv), Hsv8::new(0, 0, v));
        assert_eq!(one(gray, rgb_to_hsl), Hsl8::new(0, 0, v));
        assert_eq!(one(Hsv8::new(0, 0, v), hsv_to_rgb), gray);
        assert_eq!(one(Hsl8::new(0, 0, v), hsl_to_rgb), gray);
    }
}

#[test]
fn hue_is_ignored_when_saturation_is_zero() {
    assert_eq!(one(Hsv8::new(77, 0, 90), hsv_to_rgb), Rgb8::new(90, 90, 90));
    assert_eq!(one(Hsl8::new(77, 0, 90), hsl_to_rgb), Rgb8::new(90, 90, 90));
}

#[test]
fn hsv8_hue_never_reaches_180() {
    // Matiz 359,x° arredondaria para 180: tem de voltar a 0.
    let almost_red = one(Rgb8::new(255, 0, 1), rgb_to_hsv);
    assert_eq!(almost_red.h(), 0);
    // Um matiz de entrada fora da faixa dá a volta em vez de falhar.
    assert_eq!(
        one(Hsv8::new(180, 255, 255), hsv_to_rgb),
        Rgb8::new(255, 0, 0)
    );
    assert_eq!(
        one(Hsv8::new(240, 255, 255), hsv_to_rgb),
        Rgb8::new(0, 255, 0)
    );
}

#[test]
fn value_is_the_largest_channel_and_survives_exactly() {
    let hsv = one(Rgb8::new(10, 200, 37), rgb_to_hsv);
    assert_eq!(hsv.v(), 200);
    // S = (200 − 10) / 200 = 0.95 → 242.25.
    assert_eq!(hsv.s(), 242);
}

#[test]
fn hsl_distinguishes_tint_from_shade() {
    // Rosa claro e vinho têm o mesmo matiz; só a luminosidade muda.
    let light = one(Rgb8::new(255, 128, 128), rgb_to_hsl);
    let dark = one(Rgb8::new(128, 0, 0), rgb_to_hsl);
    assert_eq!((light.h(), dark.h()), (0, 0));
    assert_eq!((light.s(), dark.s()), (255, 255));
    assert!(light.l() > 128 && dark.l() < 128);
}

// ── Valores canônicos, f32 ─────────────────────────────────────────────────────

#[test]
fn primaries_and_secondaries_in_f32() {
    for ((r, g, b), degrees) in HUES {
        let rgb = RgbF32::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
        );
        let hsv = one(rgb, rgb_to_hsv);
        assert!(close(hsv.h(), f32::from(degrees)), "{hsv:?}");
        assert!(close(hsv.s(), 1.0) && close(hsv.v(), 1.0), "{hsv:?}");
        let hsl = one(rgb, rgb_to_hsl);
        assert!(close(hsl.h(), f32::from(degrees)), "{hsl:?}");
        assert!(close(hsl.s(), 1.0) && close(hsl.l(), 0.5), "{hsl:?}");
    }
}

#[test]
fn f32_grays_and_wrapping_hue() {
    assert_eq!(
        one(RgbF32::new(0.3, 0.3, 0.3), rgb_to_hsv),
        HsvF32::new(0.0, 0.0, 0.3)
    );
    assert_eq!(
        one(RgbF32::new(0.3, 0.3, 0.3), rgb_to_hsl),
        HslF32::new(0.0, 0.0, 0.3)
    );
    // 480° ≡ 120° e −120° ≡ 240°.
    let green = one(HsvF32::new(480.0, 1.0, 1.0), hsv_to_rgb);
    assert!(close(green.r(), 0.0) && close(green.g(), 1.0) && close(green.b(), 0.0));
    let blue = one(HslF32::new(-120.0, 1.0, 0.5), hsl_to_rgb);
    assert!(close(blue.r(), 0.0) && close(blue.g(), 0.0) && close(blue.b(), 1.0));
}

#[test]
fn f32_hue_stays_below_360() {
    let hsv = one(RgbF32::new(1.0, 0.0, 1e-7), rgb_to_hsv);
    assert!((0.0..360.0).contains(&hsv.h()), "{hsv:?}");
}

// ── Round-trips ────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn rgb8_survives_hsv8_within_the_documented_tolerance(
        image in any_image::<Rgb8>(1..=24, 1..=24),
    ) {
        let hsv = rgb_to_hsv(image.view()).unwrap();
        prop_assert!(hsv.as_pixels().iter().all(|p| p.h() < 180));
        let back = hsv_to_rgb(hsv.view()).unwrap();
        assert_images_eq!(back, image, tolerance = f64::from(HSV8_ROUND_TRIP_TOLERANCE));
    }

    #[test]
    fn rgb8_survives_hsl8_within_the_documented_tolerance(
        image in any_image::<Rgb8>(1..=24, 1..=24),
    ) {
        let hsl = rgb_to_hsl(image.view()).unwrap();
        prop_assert!(hsl.as_pixels().iter().all(|p| p.h() < 180));
        let back = hsl_to_rgb(hsl.view()).unwrap();
        assert_images_eq!(back, image, tolerance = f64::from(HSL8_ROUND_TRIP_TOLERANCE));
    }

    #[test]
    fn rgbf32_survives_hsv_and_hsl(image in any_image::<RgbF32>(1..=24, 1..=24)) {
        let tolerance = f64::from(F32_ROUND_TRIP_TOLERANCE);
        let hsv = rgb_to_hsv(image.view()).unwrap();
        prop_assert!(hsv.as_pixels().iter().all(|p| (0.0..360.0).contains(&p.h())));
        assert_images_eq!(hsv_to_rgb(hsv.view()).unwrap(), image, tolerance = tolerance);
        let hsl = rgb_to_hsl(image.view()).unwrap();
        prop_assert!(hsl.as_pixels().iter().all(|p| (0.0..360.0).contains(&p.h())));
        assert_images_eq!(hsl_to_rgb(hsl.view()).unwrap(), image, tolerance = tolerance);
    }

    #[test]
    fn saturation_and_value_stay_in_range(image in any_image::<RgbF32>(1..=16, 1..=16)) {
        for p in rgb_to_hsv(image.view()).unwrap().as_pixels() {
            prop_assert!((0.0..=1.0).contains(&p.s()) && (0.0..=1.0).contains(&p.v()));
        }
        for p in rgb_to_hsl(image.view()).unwrap().as_pixels() {
            prop_assert!((0.0..=1.0).contains(&p.s()) && (0.0..=1.0).contains(&p.l()));
        }
    }

    #[test]
    fn a_roi_converts_like_the_same_region_of_the_whole(
        image in any_image::<Rgb8>(4..=16, 4..=16),
    ) {
        let rect = Rect::new(1, 1, image.width() - 2, image.height() - 2);
        let whole = rgb_to_hsv(image.view()).unwrap();
        let part = rgb_to_hsv(image.roi(rect).unwrap()).unwrap();
        for (got, want) in part.rows().zip(whole.roi(rect).unwrap().rows()) {
            prop_assert_eq!(got, want);
        }
    }
}
