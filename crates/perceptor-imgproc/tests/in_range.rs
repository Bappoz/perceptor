//! Máscara por faixa de cor: limites por canal, matiz que dá a volta e golden.
//!
//! Regenerar os goldens: `PERCEPTOR_UPDATE_GOLDEN=1 cargo test -p perceptor-imgproc --test in_range`

use perceptor_core::{Hsv8, HsvF32, Image, Rect, Rgb8, L16, L8, LF32};
use perceptor_imgproc::hsv::rgb_to_hsv;
use perceptor_imgproc::range::in_range;
use perceptor_testkit::assert_golden;
use perceptor_testkit::patterns::noise;
use perceptor_testkit::strategies::any_image;
use proptest::prelude::*;

fn l8_values(image: &Image<L8>) -> Vec<u8> {
    image.as_pixels().iter().map(|p| p.luma()).collect()
}

// ── Faixa por canal ────────────────────────────────────────────────────────────

#[test]
fn bounds_are_inclusive_and_every_channel_must_hold() {
    let pixels = [
        Rgb8::new(10, 20, 30), // no limite inferior dos três
        Rgb8::new(50, 60, 70), // no limite superior dos três
        Rgb8::new(30, 40, 50), // dentro
        Rgb8::new(9, 40, 50),  // R abaixo
        Rgb8::new(30, 61, 50), // G acima
        Rgb8::new(30, 40, 71), // B acima
    ];
    let image = Image::from_pixels(pixels.len(), 1, &pixels).unwrap();
    let mask = in_range(image.view(), Rgb8::new(10, 20, 30), Rgb8::new(50, 60, 70)).unwrap();
    assert_eq!(l8_values(&mask), [255, 255, 255, 0, 0, 0]);
}

#[test]
fn a_full_range_matches_everything_and_an_empty_one_nothing() {
    let image = noise::<Rgb8>(12, 8, 3);
    let all = in_range(image.view(), Rgb8::new(0, 0, 0), Rgb8::new(255, 255, 255)).unwrap();
    assert!(l8_values(&all).iter().all(|&v| v == 255));
    // Faixa invertida em um canal que não é matiz: vazia, sem dar a volta.
    let none = in_range(image.view(), Rgb8::new(200, 0, 0), Rgb8::new(100, 255, 255)).unwrap();
    assert!(l8_values(&none).iter().all(|&v| v == 0));
}

#[test]
fn works_for_single_channel_and_wider_depths() {
    let gray = Image::from_fn(4, 1, |x, _| L8::new([5, 50, 100, 200][x])).unwrap();
    let mask = in_range(gray.view(), L8::new(50), L8::new(100)).unwrap();
    assert_eq!(l8_values(&mask), [0, 255, 255, 0]);

    let wide = Image::from_fn(3, 1, |x, _| L16::new([100, 40_000, 65_535][x])).unwrap();
    let mask = in_range(wide.view(), L16::new(1_000), L16::new(65_000)).unwrap();
    assert_eq!(l8_values(&mask), [0, 255, 0]);

    let float = Image::from_fn(3, 1, |x, _| LF32::new([0.1, 0.5, 0.9][x])).unwrap();
    let mask = in_range(float.view(), LF32::new(0.4), LF32::new(0.6)).unwrap();
    assert_eq!(l8_values(&mask), [0, 255, 0]);
}

// ── Matiz que dá a volta ───────────────────────────────────────────────────────

#[test]
fn a_reversed_hue_range_wraps_around_in_8_bit_hsv() {
    // Vermelho: matiz 170..180 e 0..10 (escala 0..180).
    let hues = [0u8, 5, 10, 11, 100, 169, 170, 175, 179];
    let pixels: Vec<Hsv8> = hues.iter().map(|&h| Hsv8::new(h, 200, 200)).collect();
    let image = Image::from_pixels(pixels.len(), 1, &pixels).unwrap();

    let mask = in_range(
        image.view(),
        Hsv8::new(170, 100, 50),
        Hsv8::new(10, 255, 255),
    )
    .unwrap();

    assert_eq!(l8_values(&mask), [255, 255, 255, 0, 0, 0, 255, 255, 255]);
}

#[test]
fn a_hue_range_that_does_not_wrap_is_a_plain_interval() {
    let hues = [20u8, 30, 40, 60, 61];
    let pixels: Vec<Hsv8> = hues.iter().map(|&h| Hsv8::new(h, 255, 255)).collect();
    let image = Image::from_pixels(pixels.len(), 1, &pixels).unwrap();
    let mask = in_range(image.view(), Hsv8::new(30, 0, 0), Hsv8::new(60, 255, 255)).unwrap();
    assert_eq!(l8_values(&mask), [0, 255, 255, 255, 0]);
}

#[test]
fn the_wrap_applies_only_to_the_hue_channel() {
    // Saturação invertida (200 > 100): nenhum pixel serve, matiz à parte.
    let image = Image::filled(3, 1, Hsv8::new(5, 150, 200)).unwrap();
    let mask = in_range(image.view(), Hsv8::new(0, 200, 0), Hsv8::new(180, 100, 255)).unwrap();
    assert!(l8_values(&mask).iter().all(|&v| v == 0));
}

#[test]
fn a_reversed_hue_range_wraps_in_degrees_for_f32_hsv() {
    let hues = [0.0f32, 5.0, 10.0, 11.0, 180.0, 349.0, 350.0, 359.0];
    let pixels: Vec<HsvF32> = hues.iter().map(|&h| HsvF32::new(h, 0.8, 0.8)).collect();
    let image = Image::from_pixels(pixels.len(), 1, &pixels).unwrap();

    let mask = in_range(
        image.view(),
        HsvF32::new(350.0, 0.5, 0.0),
        HsvF32::new(10.0, 1.0, 1.0),
    )
    .unwrap();

    assert_eq!(l8_values(&mask), [255, 255, 255, 0, 0, 0, 255, 255]);
}

proptest! {
    #[test]
    fn rgb_ranges_match_the_naive_definition(
        image in any_image::<Rgb8>(1..=16, 1..=16),
        lo in prop::array::uniform3(any::<u8>()),
        hi in prop::array::uniform3(any::<u8>()),
    ) {
        let mask = in_range(image.view(), Rgb8(lo), Rgb8(hi)).unwrap();
        for (pixel, out) in image.as_pixels().iter().zip(mask.as_pixels()) {
            let inside = (0..3).all(|c| lo[c] <= pixel.0[c] && pixel.0[c] <= hi[c]);
            prop_assert_eq!(out.luma(), if inside { 255 } else { 0 });
        }
    }

    #[test]
    fn hsv_ranges_match_the_naive_definition_including_the_wrap(
        image in any_image::<Hsv8>(1..=16, 1..=16),
        lo in prop::array::uniform3(any::<u8>()),
        hi in prop::array::uniform3(any::<u8>()),
    ) {
        let mask = in_range(image.view(), Hsv8(lo), Hsv8(hi)).unwrap();
        for (pixel, out) in image.as_pixels().iter().zip(mask.as_pixels()) {
            let h = pixel.0[0];
            let hue_ok = if lo[0] <= hi[0] { lo[0] <= h && h <= hi[0] } else { h >= lo[0] || h <= hi[0] };
            let rest_ok = (1..3).all(|c| lo[c] <= pixel.0[c] && pixel.0[c] <= hi[c]);
            prop_assert_eq!(out.luma(), if hue_ok && rest_ok { 255 } else { 0 });
        }
    }
}

#[test]
fn a_strided_roi_gives_the_mask_of_its_copy() {
    let image = noise::<Rgb8>(20, 12, 5);
    let view = image.roi(Rect::new(3, 2, 9, 7)).unwrap();
    let (lo, hi) = (Rgb8::new(40, 40, 40), Rgb8::new(200, 200, 200));
    let a = in_range(view, lo, hi).unwrap();
    let b = in_range(view.to_image().unwrap().view(), lo, hi).unwrap();
    assert_eq!(a.as_pixels(), b.as_pixels());
}

// ── Golden: objetos coloridos sintéticos ───────────────────────────────────────

/// 96×32, fundo cinza com um disco vermelho, um verde, um azul e um amarelo.
fn colored_objects() -> Image<Rgb8> {
    let discs = [
        (12usize, Rgb8::new(255, 0, 0)),
        (36, Rgb8::new(0, 255, 0)),
        (60, Rgb8::new(0, 0, 255)),
        (84, Rgb8::new(255, 255, 0)),
    ];
    Image::from_fn(96, 32, |x, y| {
        for (cx, color) in discs {
            let (dx, dy) = (x.abs_diff(cx), y.abs_diff(16));
            if dx * dx + dy * dy < 10 * 10 {
                return color;
            }
        }
        Rgb8::new(90, 90, 90)
    })
    .unwrap()
}

#[test]
fn golden_input_objects() {
    assert_golden!(colored_objects(), "in_range_objects_input.png");
}

#[test]
fn golden_red_mask_uses_the_hue_wrap() {
    let hsv = rgb_to_hsv::<Rgb8>(colored_objects().view()).unwrap();
    // Vermelho cruza o zero da roda: 170..=179 ou 0..=10.
    let mask = in_range(hsv.view(), Hsv8::new(170, 100, 50), Hsv8::new(10, 255, 255)).unwrap();
    assert_golden!(mask, "in_range_objects_red.png");
}

#[test]
fn golden_green_mask() {
    let hsv = rgb_to_hsv::<Rgb8>(colored_objects().view()).unwrap();
    let mask = in_range(hsv.view(), Hsv8::new(50, 100, 50), Hsv8::new(70, 255, 255)).unwrap();
    assert_golden!(mask, "in_range_objects_green.png");
}
