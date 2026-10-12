//! Propriedades da conversão para escala de cinza em ponto fixo.

use perceptor_core::{Image, Rect, Rgb8, L8};
use perceptor_imgproc::grayscale::{convert_to_gray, convert_to_gray_with, reference, LumaWeights};
use perceptor_testkit::assert_images_eq;
use perceptor_testkit::patterns::noise;
use perceptor_testkit::strategies::any_image;
use proptest::prelude::*;

const WEIGHTS: [LumaWeights; 2] = [LumaWeights::Bt601, LumaWeights::Bt709];

fn weights() -> impl Strategy<Value = LumaWeights> {
    prop::sample::select(WEIGHTS.to_vec())
}

fn gray_of(pixel: Rgb8, weights: LumaWeights) -> u8 {
    let input = Image::filled(1, 1, pixel).unwrap();
    convert_to_gray_with(input.view(), weights)
        .unwrap()
        .as_pixels()[0]
        .luma()
}

proptest! {
    #[test]
    fn stays_within_one_level_of_the_f64_reference(
        image in any_image::<Rgb8>(1..=48, 1..=48),
        weights in weights(),
    ) {
        let fast = convert_to_gray_with(image.view(), weights).unwrap();
        let exact = reference::convert_to_gray(image.view(), weights).unwrap();
        assert_images_eq!(fast, exact, tolerance = 1.0);
    }

    #[test]
    fn gray_input_is_preserved_exactly(v in any::<u8>(), weights in weights()) {
        prop_assert_eq!(gray_of(Rgb8::new(v, v, v), weights), v);
    }

    #[test]
    fn output_is_monotonic_in_each_channel(
        r in any::<u8>(), g in any::<u8>(), b in any::<u8>(), weights in weights(),
    ) {
        let base = gray_of(Rgb8::new(r, g, b), weights);
        prop_assert!(gray_of(Rgb8::new(r.saturating_add(1), g, b), weights) >= base);
        prop_assert!(gray_of(Rgb8::new(r, g.saturating_add(1), b), weights) >= base);
        prop_assert!(gray_of(Rgb8::new(r, g, b.saturating_add(1)), weights) >= base);
    }

    #[test]
    fn a_roi_gives_the_same_pixels_as_the_whole_image(
        image in any_image::<Rgb8>(4..=24, 4..=24),
        weights in weights(),
    ) {
        let whole = convert_to_gray_with(image.view(), weights).unwrap();
        let rect = Rect::new(1, 2, image.width() - 2, image.height() - 3);

        let part = convert_to_gray_with(image.roi(rect).unwrap(), weights).unwrap();

        let expected = whole.roi(rect).unwrap();
        for (got, want) in part.rows().zip(expected.rows()) {
            prop_assert_eq!(got, want);
        }
    }
}

#[test]
fn default_weights_are_bt601() {
    assert_eq!(LumaWeights::default(), LumaWeights::Bt601);
    let image = noise::<Rgb8>(31, 17, 3);
    let default = convert_to_gray(image.view()).unwrap();
    let explicit = convert_to_gray_with(image.view(), LumaWeights::Bt601).unwrap();
    assert_images_eq!(default, explicit);
}

#[test]
fn result_is_rounded_not_truncated() {
    // 0.587 · 255 = 149.685: truncar dá 149.
    assert_eq!(gray_of(Rgb8::new(0, 255, 0), LumaWeights::Bt601), 150);
    // 0.299·255 + 0.587·255 + 0.114·254 = 254.886.
    assert_eq!(gray_of(Rgb8::new(255, 255, 254), LumaWeights::Bt601), 255);
}

#[test]
fn bt709_weighs_green_more_than_bt601() {
    // 0.7152 · 255 = 182.376; 0.2126 · 255 = 54.213; 0.0722 · 255 = 18.411.
    assert_eq!(gray_of(Rgb8::new(0, 255, 0), LumaWeights::Bt709), 182);
    assert_eq!(gray_of(Rgb8::new(255, 0, 0), LumaWeights::Bt709), 54);
    assert_eq!(gray_of(Rgb8::new(0, 0, 255), LumaWeights::Bt709), 18);
}

#[test]
fn coefficients_sum_to_one() {
    for weights in WEIGHTS {
        let sum: f64 = weights.coefficients().iter().sum();
        assert!((sum - 1.0).abs() < 1e-12, "{weights:?}: {sum}");
    }
}

/// Imagem grande o bastante para o caminho paralelo: cada linha, convertida
/// sozinha, tem de dar o mesmo que na imagem inteira, e tudo fica a um nível
/// da referência.
#[test]
fn parallel_strips_match_row_by_row_conversion() {
    let image = noise::<Rgb8>(701, 397, 11);
    for weights in WEIGHTS {
        let whole = convert_to_gray_with(image.view(), weights).unwrap();

        for y in [0, 1, 198, 395, 396] {
            let row = image.roi(Rect::new(0, y, image.width(), 1)).unwrap();
            let alone = convert_to_gray_with(row, weights).unwrap();
            assert_eq!(alone.row(0), whole.row(y), "linha {y}");
        }
        let exact = reference::convert_to_gray(image.view(), weights).unwrap();
        assert_images_eq!(whole, exact, tolerance = 1.0);
    }
}

#[test]
fn reference_rounds_the_f64_luma() {
    let image = Image::from_pixels(2, 1, &[Rgb8::new(0, 255, 0), Rgb8::new(10, 20, 30)]).unwrap();
    let gray = reference::convert_to_gray(image.view(), LumaWeights::Bt601).unwrap();
    // 0.299·10 + 0.587·20 + 0.114·30 = 18.15.
    assert_eq!(gray.as_pixels(), [L8::new(150), L8::new(18)]);
}
