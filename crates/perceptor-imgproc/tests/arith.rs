//! Aritmética saturada e lógica bit a bit entre imagens, com máscara opcional.

// Os esperados em `f32` são exatos em binário (0,5, 1,5, 100,0…).
#![allow(clippy::float_cmp)]

use perceptor_core::{Error, Image, Rgb8, L16, L8, LF32};
use perceptor_imgproc::arith::{
    absdiff, add, add_weighted, bitwise_and, bitwise_not, bitwise_or, bitwise_xor, sub,
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

fn lf32(values: &[f32]) -> Image<LF32> {
    Image::from_fn(values.len(), 1, |x, _| LF32::new(values[x])).unwrap()
}

// ── Saturação e valores à mão ──────────────────────────────────────────────────

#[test]
fn add_saturates_at_the_top() {
    let out = add(l8(&[10, 200, 255]).view(), l8(&[20, 100, 1]).view(), None).unwrap();
    assert_eq!(l8_values(&out), [30, 255, 255]);
}

#[test]
fn sub_saturates_at_zero_and_is_not_symmetric() {
    let (a, b) = (l8(&[10, 200, 5]), l8(&[20, 100, 5]));
    assert_eq!(
        l8_values(&sub(a.view(), b.view(), None).unwrap()),
        [0, 100, 0]
    );
    assert_eq!(
        l8_values(&sub(b.view(), a.view(), None).unwrap()),
        [10, 0, 0]
    );
}

#[test]
fn absdiff_is_symmetric_and_never_saturates() {
    let (a, b) = (l8(&[10, 200, 0]), l8(&[20, 100, 255]));
    let ab = absdiff(a.view(), b.view(), None).unwrap();
    let ba = absdiff(b.view(), a.view(), None).unwrap();
    assert_eq!(l8_values(&ab), [10, 100, 255]);
    assert_eq!(l8_values(&ab), l8_values(&ba));
}

#[test]
fn sixteen_bit_images_saturate_at_65535() {
    let a = Image::from_fn(2, 1, |x, _| L16::new([60_000, 100][x])).unwrap();
    let b = Image::from_fn(2, 1, |x, _| L16::new([10_000, 200][x])).unwrap();
    let sum = add(a.view(), b.view(), None).unwrap();
    assert_eq!(
        sum.as_pixels().iter().map(|p| p.luma()).collect::<Vec<_>>(),
        [65_535, 300]
    );
    let diff = sub(a.view(), b.view(), None).unwrap();
    assert_eq!(
        diff.as_pixels()
            .iter()
            .map(|p| p.luma())
            .collect::<Vec<_>>(),
        [50_000, 0]
    );
}

#[test]
fn float_images_are_not_clamped() {
    let (a, b) = (lf32(&[0.75, 0.25]), lf32(&[0.5, 0.5]));
    let sum = add(a.view(), b.view(), None).unwrap();
    assert_eq!(
        sum.as_pixels().iter().map(|p| p.luma()).collect::<Vec<_>>(),
        [1.25, 0.75]
    );
    let diff = sub(a.view(), b.view(), None).unwrap();
    assert_eq!(
        diff.as_pixels()
            .iter()
            .map(|p| p.luma())
            .collect::<Vec<_>>(),
        [0.25, -0.25]
    );
    let abs = absdiff(a.view(), b.view(), None).unwrap();
    assert_eq!(
        abs.as_pixels().iter().map(|p| p.luma()).collect::<Vec<_>>(),
        [0.25, 0.25]
    );
}

#[test]
fn each_channel_is_processed_on_its_own() {
    let a = Image::filled(1, 1, Rgb8::new(250, 10, 100)).unwrap();
    let b = Image::filled(1, 1, Rgb8::new(10, 20, 100)).unwrap();
    assert_eq!(
        add(a.view(), b.view(), None).unwrap().as_pixels()[0].0,
        [255, 30, 200]
    );
    assert_eq!(
        sub(a.view(), b.view(), None).unwrap().as_pixels()[0].0,
        [240, 0, 0]
    );
}

// ── add_weighted ───────────────────────────────────────────────────────────────

#[test]
fn add_weighted_blends_with_rounding() {
    // 0,5·100 + 0,5·201 = 150,5 → 151 (metade para longe de zero).
    let out = add_weighted(l8(&[100]).view(), 0.5, l8(&[201]).view(), 0.5, 0.0, None).unwrap();
    assert_eq!(l8_values(&out), [151]);
}

#[test]
fn add_weighted_applies_gamma_and_saturates() {
    let (a, b) = (l8(&[100, 200]), l8(&[100, 200]));
    // 1·a + 1·b + (−50): 150 e 255 (350 satura).
    let out = add_weighted(a.view(), 1.0, b.view(), 1.0, -50.0, None).unwrap();
    assert_eq!(l8_values(&out), [150, 255]);
    // Pesos negativos: 2·a − b = 100 e 200.
    let out = add_weighted(a.view(), 2.0, b.view(), -1.0, 0.0, None).unwrap();
    assert_eq!(l8_values(&out), [100, 200]);
    // Saturação para baixo.
    let out = add_weighted(a.view(), 1.0, b.view(), -2.0, 0.0, None).unwrap();
    assert_eq!(l8_values(&out), [0, 0]);
}

#[test]
fn a_blend_with_weights_summing_to_one_stays_between_the_operands() {
    let a = noise::<L8>(16, 12, 1);
    let b = noise::<L8>(16, 12, 2);
    let out = add_weighted(a.view(), 0.3, b.view(), 0.7, 0.0, None).unwrap();
    for ((x, y), z) in a.as_pixels().iter().zip(b.as_pixels()).zip(out.as_pixels()) {
        let (lo, hi) = (x.luma().min(y.luma()), x.luma().max(y.luma()));
        assert!(
            (lo..=hi).contains(&z.luma()),
            "{} {} → {}",
            x.luma(),
            y.luma(),
            z.luma()
        );
    }
}

// ── Lógica bit a bit ───────────────────────────────────────────────────────────

#[test]
fn bitwise_operations_match_the_truth_table() {
    let (a, b) = (
        l8(&[0b1111_0000, 0b0011_1100]),
        l8(&[0b0011_1100, 0b0000_1111]),
    );
    assert_eq!(
        l8_values(&bitwise_and(a.view(), b.view(), None).unwrap()),
        [0b0011_0000, 0b0000_1100]
    );
    assert_eq!(
        l8_values(&bitwise_or(a.view(), b.view(), None).unwrap()),
        [0b1111_1100, 0b0011_1111]
    );
    assert_eq!(
        l8_values(&bitwise_xor(a.view(), b.view(), None).unwrap()),
        [0b1100_1100, 0b0011_0011]
    );
    assert_eq!(
        l8_values(&bitwise_not(a.view(), None).unwrap()),
        [0b0000_1111, 0b1100_0011]
    );
}

#[test]
fn bitwise_works_on_sixteen_bits() {
    let a = Image::filled(1, 1, L16::new(0xFF00)).unwrap();
    let b = Image::filled(1, 1, L16::new(0x0FF0)).unwrap();
    assert_eq!(
        bitwise_and(a.view(), b.view(), None).unwrap().as_pixels()[0].luma(),
        0x0F00
    );
    assert_eq!(
        bitwise_not(a.view(), None).unwrap().as_pixels()[0].luma(),
        0x00FF
    );
}

proptest! {
    #[test]
    fn de_morgan_holds(
        a in any_image::<Rgb8>(1..=12, 1..=12),
        seed in any::<u64>(),
    ) {
        let b = noise::<Rgb8>(a.width(), a.height(), seed);
        let left = bitwise_not(bitwise_and(a.view(), b.view(), None).unwrap().view(), None).unwrap();
        let (na, nb) = (
            bitwise_not(a.view(), None).unwrap(),
            bitwise_not(b.view(), None).unwrap(),
        );
        let right = bitwise_or(na.view(), nb.view(), None).unwrap();
        prop_assert_eq!(left.as_pixels(), right.as_pixels());
    }

    #[test]
    fn xor_with_itself_is_zero_and_not_not_is_the_identity(a in any_image::<L8>(1..=12, 1..=12)) {
        let zero = bitwise_xor(a.view(), a.view(), None).unwrap();
        prop_assert!(l8_values(&zero).iter().all(|&v| v == 0));
        let twice = bitwise_not(bitwise_not(a.view(), None).unwrap().view(), None).unwrap();
        prop_assert_eq!(twice.as_pixels(), a.as_pixels());
    }
}

// ── Contra a referência ingênua ────────────────────────────────────────────────

fn naive(
    first: &Image<L8>,
    second: &Image<L8>,
    mask: Option<&Image<L8>>,
    op: impl Fn(i32, i32) -> i32,
) -> Vec<u8> {
    let on = |i: usize| mask.is_none_or(|m| m.as_pixels()[i].luma() != 0);
    (0..first.as_pixels().len())
        .map(|i| {
            let lhs = i32::from(first.as_pixels()[i].luma());
            let rhs = i32::from(second.as_pixels()[i].luma());
            if on(i) {
                u8::try_from(op(lhs, rhs).clamp(0, 255)).unwrap()
            } else {
                first.as_pixels()[i].luma()
            }
        })
        .collect()
}

proptest! {
    #[test]
    fn arithmetic_matches_the_naive_implementation(
        a in any_image::<L8>(1..=16, 1..=16),
        seed in any::<u64>(),
        masked in any::<bool>(),
    ) {
        let b = noise::<L8>(a.width(), a.height(), seed);
        let mask = noise::<L8>(a.width(), a.height(), seed ^ 0x9E37_79B9);
        let m = masked.then_some(&mask);
        let view = m.map(Image::view);

        prop_assert_eq!(
            l8_values(&add(a.view(), b.view(), view).unwrap()),
            naive(&a, &b, m, |x, y| x + y)
        );
        prop_assert_eq!(
            l8_values(&sub(a.view(), b.view(), view).unwrap()),
            naive(&a, &b, m, |x, y| x - y)
        );
        prop_assert_eq!(
            l8_values(&absdiff(a.view(), b.view(), view).unwrap()),
            naive(&a, &b, m, |x, y| (x - y).abs())
        );
        prop_assert_eq!(
            l8_values(&bitwise_and(a.view(), b.view(), view).unwrap()),
            naive(&a, &b, m, |x, y| x & y)
        );
        prop_assert_eq!(
            l8_values(&bitwise_or(a.view(), b.view(), view).unwrap()),
            naive(&a, &b, m, |x, y| x | y)
        );
        prop_assert_eq!(
            l8_values(&bitwise_xor(a.view(), b.view(), view).unwrap()),
            naive(&a, &b, m, |x, y| x ^ y)
        );
    }

    #[test]
    fn add_weighted_matches_the_naive_formula(
        a in any_image::<L8>(1..=12, 1..=12),
        seed in any::<u64>(),
        alpha in -2.0f64..2.0,
        beta in -2.0f64..2.0,
        gamma in -300.0f64..300.0,
    ) {
        let b = noise::<L8>(a.width(), a.height(), seed);
        let out = add_weighted(a.view(), alpha, b.view(), beta, gamma, None).unwrap();
        for ((x, y), z) in a.as_pixels().iter().zip(b.as_pixels()).zip(out.as_pixels()) {
            let expect = (alpha * f64::from(x.luma()) + beta * f64::from(y.luma()) + gamma)
                .round()
                .clamp(0.0, 255.0);
            prop_assert!((f64::from(z.luma()) - expect).abs() < 0.5);
        }
    }
}

// ── Máscara ────────────────────────────────────────────────────────────────────

#[test]
fn outside_the_mask_the_first_operand_is_kept() {
    let (a, b) = (l8(&[10, 20, 30, 40]), l8(&[1, 1, 1, 1]));
    let mask = l8(&[255, 0, 1, 0]);
    let out = add(a.view(), b.view(), Some(mask.view())).unwrap();
    assert_eq!(l8_values(&out), [11, 20, 31, 40]);
    let out = bitwise_not(a.view(), Some(mask.view())).unwrap();
    assert_eq!(l8_values(&out), [245, 20, 225, 40]);
}

#[test]
fn an_all_zero_mask_returns_the_first_operand() {
    let a = noise::<L8>(8, 8, 1);
    let b = noise::<L8>(8, 8, 2);
    let mask = Image::filled(8, 8, L8::new(0)).unwrap();
    let out = absdiff(a.view(), b.view(), Some(mask.view())).unwrap();
    assert_eq!(out.as_pixels(), a.as_pixels());
}

#[test]
fn the_mask_applies_to_every_channel_of_a_pixel() {
    let a = Image::filled(2, 1, Rgb8::new(1, 2, 3)).unwrap();
    let b = Image::filled(2, 1, Rgb8::new(10, 10, 10)).unwrap();
    let mask = l8(&[255, 0]);
    let out = add(a.view(), b.view(), Some(mask.view())).unwrap();
    assert_eq!(out.as_pixels()[0].0, [11, 12, 13]);
    assert_eq!(out.as_pixels()[1].0, [1, 2, 3]);
}

// ── Erros ──────────────────────────────────────────────────────────────────────

#[test]
fn operands_of_different_sizes_are_rejected() {
    let (a, b) = (noise::<L8>(8, 6, 1), noise::<L8>(8, 5, 1));
    let shape = |err: Error| {
        matches!(
            err,
            Error::ShapeMismatch {
                expected: (8, 6),
                actual: (8, 5)
            }
        )
    };
    assert!(shape(add(a.view(), b.view(), None).unwrap_err()));
    assert!(shape(sub(a.view(), b.view(), None).unwrap_err()));
    assert!(shape(absdiff(a.view(), b.view(), None).unwrap_err()));
    assert!(shape(bitwise_and(a.view(), b.view(), None).unwrap_err()));
    assert!(shape(bitwise_or(a.view(), b.view(), None).unwrap_err()));
    assert!(shape(bitwise_xor(a.view(), b.view(), None).unwrap_err()));
    assert!(shape(
        add_weighted(a.view(), 1.0, b.view(), 1.0, 0.0, None).unwrap_err()
    ));
}

#[test]
fn a_mask_of_another_size_is_rejected() {
    let a = noise::<L8>(8, 6, 1);
    let mask = noise::<L8>(7, 6, 1);
    let err = add(a.view(), a.view(), Some(mask.view())).unwrap_err();
    assert!(
        matches!(
            err,
            Error::ShapeMismatch {
                expected: (8, 6),
                actual: (7, 6)
            }
        ),
        "{err:?}"
    );
    let err = bitwise_not(a.view(), Some(mask.view())).unwrap_err();
    assert!(matches!(err, Error::ShapeMismatch { .. }), "{err:?}");
}
