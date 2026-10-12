//! Limiarização adaptativa: média e gaussiana locais, bordas e goldens.
//!
//! Regenerar os goldens: `PERCEPTOR_UPDATE_GOLDEN=1 cargo test -p perceptor-imgproc --test adaptive`

use perceptor_core::{Error, Image, L8};
use perceptor_imgproc::adaptive::{adaptive_threshold, adaptive_threshold_inv, AdaptiveMethod};
use perceptor_imgproc::threshold::{threshold_otsu, ThresholdType};
use perceptor_testkit::strategies::any_image;
use perceptor_testkit::{assert_golden, assert_images_eq};
use proptest::prelude::*;

const METHODS: [AdaptiveMethod; 2] = [AdaptiveMethod::Mean, AdaptiveMethod::Gaussian];

fn levels(image: &Image<L8>) -> Vec<u8> {
    image.as_pixels().iter().map(|p| p.luma()).collect()
}

/// Referência ingênua da média: janela `block × block` com borda replicada,
/// aritmética inteira exata. `true` onde `v + c > média`.
fn naive_mean_above(image: &Image<L8>, block: usize, c: i32) -> Vec<bool> {
    let (width, height) = (image.width(), image.height());
    let radius = i64::try_from(block / 2).unwrap();
    let area = i64::try_from(block * block).unwrap();
    let at = |x: i64, y: i64| -> i64 {
        let x = usize::try_from(x.clamp(0, i64::try_from(width).unwrap() - 1)).unwrap();
        let y = usize::try_from(y.clamp(0, i64::try_from(height).unwrap() - 1)).unwrap();
        i64::from(image.as_pixels()[y * width + x].luma())
    };
    let mut out = Vec::with_capacity(width * height);
    for y in 0..i64::try_from(height).unwrap() {
        for x in 0..i64::try_from(width).unwrap() {
            let sum: i64 = (-radius..=radius)
                .flat_map(|dy| (-radius..=radius).map(move |dx| (dx, dy)))
                .map(|(dx, dy)| at(x + dx, y + dy))
                .sum();
            out.push((at(x, y) + i64::from(c)) * area > sum);
        }
    }
    out
}

/// Fundo com gradiente de iluminação (60 → 200 da esquerda para a direita) e
/// traços verticais 50 níveis mais escuros. Devolve a imagem e a máscara
/// ideal: `255` no fundo, `0` nos traços.
fn illuminated_strokes() -> (Image<L8>, Image<L8>) {
    const W: usize = 64;
    const H: usize = 24;
    let background = |x: usize| u8::try_from(60 + x * 140 / (W - 1)).unwrap();
    let is_stroke = |x: usize| matches!(x % 16, 7 | 8);
    let image = Image::from_fn(W, H, |x, _| {
        let bg = background(x);
        L8::new(if is_stroke(x) { bg - 50 } else { bg })
    })
    .unwrap();
    let mask = Image::from_fn(W, H, |x, _| L8::new(if is_stroke(x) { 0 } else { 255 })).unwrap();
    (image, mask)
}

// ── Parâmetros ─────────────────────────────────────────────────────────────────

#[test]
fn block_size_must_be_odd_and_greater_than_one() {
    let image = Image::filled(8, 8, L8::new(10)).unwrap();
    for method in METHODS {
        for size in [0usize, 1, 2, 4, 10] {
            let err = adaptive_threshold(image.view(), 255, method, size, 0).unwrap_err();
            assert!(
                matches!(err, Error::InvalidWindowSize { size: s } if s == size),
                "{method:?} {size}: {err:?}"
            );
            let err = adaptive_threshold_inv(image.view(), 255, method, size, 0).unwrap_err();
            assert!(matches!(err, Error::InvalidWindowSize { .. }), "{err:?}");
        }
        for size in [3usize, 5, 7, 101] {
            assert!(adaptive_threshold(image.view(), 255, method, size, 0).is_ok());
        }
    }
}

#[test]
fn output_has_the_input_dimensions() {
    let image = Image::filled(7, 3, L8::new(10)).unwrap();
    for method in METHODS {
        let out = adaptive_threshold(image.view(), 255, method, 5, 0).unwrap();
        assert_eq!((out.width(), out.height()), (7, 3));
    }
}

// ── Média: contra a referência ingênua ─────────────────────────────────────────

proptest! {
    #[test]
    fn mean_matches_the_naive_reference(
        image in any_image::<L8>(1..=20, 1..=20),
        block in prop::sample::select(vec![3usize, 5, 7, 9, 15, 31]),
        c in -30i32..=30,
        max in any::<u8>(),
    ) {
        let above = naive_mean_above(&image, block, c);

        let binary = adaptive_threshold(image.view(), max, AdaptiveMethod::Mean, block, c).unwrap();
        let inverse = adaptive_threshold_inv(image.view(), max, AdaptiveMethod::Mean, block, c).unwrap();

        let expect_binary: Vec<u8> = above.iter().map(|&a| if a { max } else { 0 }).collect();
        let expect_inverse: Vec<u8> = above.iter().map(|&a| if a { 0 } else { max }).collect();
        prop_assert_eq!(levels(&binary), expect_binary);
        prop_assert_eq!(levels(&inverse), expect_inverse);
    }

    #[test]
    fn inverse_is_the_complement_of_binary(
        image in any_image::<L8>(1..=16, 1..=16),
        block in prop::sample::select(vec![3usize, 5, 11]),
        c in -20i32..=20,
        max in any::<u8>(),
        gaussian in any::<bool>(),
    ) {
        let method = if gaussian { AdaptiveMethod::Gaussian } else { AdaptiveMethod::Mean };
        let binary = levels(&adaptive_threshold(image.view(), max, method, block, c).unwrap());
        let inverse = levels(&adaptive_threshold_inv(image.view(), max, method, block, c).unwrap());
        for (b, i) in binary.iter().zip(&inverse) {
            prop_assert_eq!(u16::from(*b) + u16::from(*i), u16::from(max));
        }
    }
}

// ── Propriedades exatas, nos dois métodos ──────────────────────────────────────

#[test]
fn a_flat_image_is_decided_by_the_sign_of_c() {
    let image = Image::filled(9, 6, L8::new(123)).unwrap();
    for method in METHODS {
        for block in [3, 5, 9, 25] {
            // `v > v − c` só vale com c > 0; com c = 0 o pixel é igual à média.
            let on = |c| levels(&adaptive_threshold(image.view(), 255, method, block, c).unwrap());
            assert!(on(1).iter().all(|&v| v == 255), "{method:?} {block} c=1");
            assert!(on(0).iter().all(|&v| v == 0), "{method:?} {block} c=0");
            assert!(on(-1).iter().all(|&v| v == 0), "{method:?} {block} c=-1");
        }
    }
}

#[test]
fn a_linear_ramp_equals_its_local_mean_away_from_the_border() {
    // Janela simétrica sobre uma rampa: a média é o próprio pixel, exatamente.
    let ramp = Image::from_fn(40, 5, |x, _| L8::new(u8::try_from(x * 3).unwrap())).unwrap();
    let block = 7;
    for method in METHODS {
        let flat = levels(&adaptive_threshold(ramp.view(), 255, method, block, 0).unwrap());
        let lifted = levels(&adaptive_threshold(ramp.view(), 255, method, block, 1).unwrap());
        for y in 0..5 {
            for x in block / 2..40 - block / 2 {
                assert_eq!(flat[y * 40 + x], 0, "{method:?} c=0 em ({x},{y})");
                assert_eq!(lifted[y * 40 + x], 255, "{method:?} c=1 em ({x},{y})");
            }
        }
    }
}

// ── Gaussiana ──────────────────────────────────────────────────────────────────

#[test]
fn gaussian_weights_the_center_more_than_the_box() {
    // Fundo 100 com um pixel 200 a um passo do pixel analisado (100), c = 5.
    // Média 5×5: 100 + 100/25 = 104 → 105 > 104, liga.
    // Gaussiana σ = 1,1: peso 2D do vizinho ≈ 0,090 → média ≈ 109 → 105 > 109 falha.
    let mut image = Image::filled(11, 11, L8::new(100)).unwrap();
    image.as_pixels_mut()[5 * 11 + 6] = L8::new(200);
    let probe = 5 * 11 + 5;

    let mean = adaptive_threshold(image.view(), 255, AdaptiveMethod::Mean, 5, 5).unwrap();
    let gauss = adaptive_threshold(image.view(), 255, AdaptiveMethod::Gaussian, 5, 5).unwrap();

    assert_eq!(levels(&mean)[probe], 255);
    assert_eq!(levels(&gauss)[probe], 0);
}

// ── Iluminação não uniforme ────────────────────────────────────────────────────

#[test]
fn adaptive_recovers_strokes_under_an_illumination_gradient() {
    let (image, mask) = illuminated_strokes();

    let out = adaptive_threshold(image.view(), 255, AdaptiveMethod::Mean, 15, 12).unwrap();

    assert_images_eq!(out, mask);
}

#[test]
fn a_global_threshold_cannot_recover_the_same_strokes() {
    let (image, mask) = illuminated_strokes();

    let (_, global) = threshold_otsu(image.view(), 255, ThresholdType::Binary).unwrap();

    let wrong = levels(&global)
        .iter()
        .zip(levels(&mask))
        .filter(|(a, b)| **a != *b)
        .count();
    assert!(
        wrong > 200,
        "só {wrong} pixels errados; o gradiente não pesou"
    );
}

#[test]
fn golden_adaptive_mean_on_illumination_gradient() {
    let (image, _) = illuminated_strokes();
    let out = adaptive_threshold(image.view(), 255, AdaptiveMethod::Mean, 15, 12).unwrap();
    assert_golden!(out, "adaptive_mean_strokes.png");
}

#[test]
fn golden_adaptive_gaussian_on_illumination_gradient() {
    let (image, _) = illuminated_strokes();
    let out = adaptive_threshold(image.view(), 255, AdaptiveMethod::Gaussian, 15, 12).unwrap();
    assert_golden!(out, "adaptive_gaussian_strokes.png");
}

#[test]
fn golden_input_image() {
    // O golden de entrada documenta o que os de saída processaram.
    let (image, _) = illuminated_strokes();
    assert_golden!(image, "adaptive_strokes_input.png");
}
