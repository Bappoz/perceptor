//! Tabelas de consulta: aplicação, gamma e ganho/offset em 8 e 16 bits.

use perceptor_core::{Error, Image, Rect, Rgb8, L16, L8};
use perceptor_imgproc::lut::{apply_lut, apply_lut_in_place, apply_lut_per_channel, Lut};
use perceptor_testkit::assert_images_eq;
use perceptor_testkit::strategies::any_image;
use proptest::prelude::*;

fn levels(image: &Image<L8>) -> Vec<u8> {
    image.as_pixels().iter().map(|p| p.luma()).collect()
}

/// Todos os 256 níveis, um por pixel.
fn ramp() -> Image<L8> {
    Image::from_fn(256, 1, |x, _| L8::new(u8::try_from(x).unwrap())).unwrap()
}

fn ramp_levels(lut: &Lut<u8>) -> Vec<u8> {
    levels(&apply_lut(ramp().view(), lut).unwrap())
}

// ── Identidade e construção ────────────────────────────────────────────────────

#[test]
fn tables_have_one_entry_per_level() {
    assert_eq!(Lut::<u8>::identity().as_slice().len(), 256);
    assert_eq!(Lut::<u16>::identity().as_slice().len(), 65_536);
}

#[test]
fn identity_maps_each_level_to_itself() {
    let table = Lut::<u8>::identity();
    assert!(table
        .as_slice()
        .iter()
        .enumerate()
        .all(|(i, &v)| usize::from(v) == i));
    let wide = Lut::<u16>::identity();
    assert!(wide
        .as_slice()
        .iter()
        .enumerate()
        .all(|(i, &v)| usize::from(v) == i));
}

#[test]
fn from_fn_receives_every_level_in_order() {
    let table = Lut::<u8>::from_fn(|v| 255 - v);
    assert_eq!(table.as_slice()[0], 255);
    assert_eq!(table.as_slice()[255], 0);
    assert_eq!(table.as_slice(), Lut::<u8>::invert().as_slice());
}

proptest! {
    #[test]
    fn identity_does_not_change_an_8_bit_image(image in any_image::<Rgb8>(1..=16, 1..=16)) {
        let out = apply_lut(image.view(), &Lut::<u8>::identity()).unwrap();
        prop_assert_eq!(out.as_pixels(), image.as_pixels());
    }

    #[test]
    fn identity_does_not_change_a_16_bit_image(image in any_image::<L16>(1..=16, 1..=16)) {
        let out = apply_lut(image.view(), &Lut::<u16>::identity()).unwrap();
        prop_assert_eq!(out.as_pixels(), image.as_pixels());
    }

    #[test]
    fn apply_maps_every_sample_through_the_table(
        image in any_image::<Rgb8>(1..=12, 1..=12),
        salt in any::<u8>(),
    ) {
        let table = Lut::<u8>::from_fn(|v| v.wrapping_mul(37).wrapping_add(salt));
        let out = apply_lut(image.view(), &table).unwrap();
        let expect: Vec<u8> = image
            .as_pixels()
            .iter()
            .flat_map(|p| p.0)
            .map(|v| v.wrapping_mul(37).wrapping_add(salt))
            .collect();
        let got: Vec<u8> = out.as_pixels().iter().flat_map(|p| p.0).collect();
        prop_assert_eq!(got, expect);
    }

    #[test]
    fn in_place_equals_out_of_place(image in any_image::<L8>(1..=16, 1..=16), gamma in 0.1f64..4.0) {
        let table = Lut::<u8>::gamma(gamma);
        let expected = apply_lut(image.view(), &table).unwrap();
        let mut copy = image.clone();
        apply_lut_in_place(copy.view_mut(), &table);
        prop_assert_eq!(copy.as_pixels(), expected.as_pixels());
    }
}

// ── Gamma ──────────────────────────────────────────────────────────────────────

#[test]
fn gamma_one_is_the_identity() {
    assert_eq!(
        Lut::<u8>::gamma(1.0).as_slice(),
        Lut::<u8>::identity().as_slice()
    );
    assert_eq!(
        Lut::<u16>::gamma(1.0).as_slice(),
        Lut::<u16>::identity().as_slice()
    );
}

#[test]
fn gamma_matches_hand_computed_values() {
    // out = round(255 · (v / 255)^γ); valores calculados à mão.
    let half = Lut::<u8>::gamma(0.5);
    assert_eq!(half.as_slice()[64], 128, "255·√(64/255) = 127,75");
    assert_eq!(half.as_slice()[1], 16, "255·√(1/255) = 15,97");
    let square = Lut::<u8>::gamma(2.0);
    assert_eq!(square.as_slice()[128], 64, "255·(128/255)² = 64,25");
    assert_eq!(square.as_slice()[255], 255);
}

#[test]
fn gamma_below_one_brightens_and_above_one_darkens() {
    let (bright, dark) = (Lut::<u8>::gamma(0.5), Lut::<u8>::gamma(2.0));
    for v in 1..=254u8 {
        assert!(bright.as_slice()[usize::from(v)] >= v, "{v}");
        assert!(dark.as_slice()[usize::from(v)] <= v, "{v}");
    }
}

#[test]
fn gamma_16_bit_matches_a_hand_computed_value() {
    // 65535 · (32768 / 65535)^2 = 16384,25
    assert_eq!(Lut::<u16>::gamma(2.0).as_slice()[32_768], 16_384);
}

proptest! {
    #[test]
    fn gamma_keeps_the_endpoints_and_is_monotonic(gamma in 0.05f64..8.0) {
        let table = Lut::<u8>::gamma(gamma);
        let s = table.as_slice();
        prop_assert_eq!(s[0], 0);
        prop_assert_eq!(s[255], 255);
        prop_assert!(s.windows(2).all(|w| w[0] <= w[1]));
    }
}

// ── Ganho e offset ─────────────────────────────────────────────────────────────

#[test]
fn gain_offset_matches_hand_computed_values() {
    let table = Lut::<u8>::gain_offset(1.5, 10.0);
    assert_eq!(table.as_slice()[100], 160);
    assert_eq!(table.as_slice()[3], 15, "14,5 arredonda para longe de zero");
    assert_eq!(table.as_slice()[0], 10);
}

#[test]
fn gain_offset_identity_values_give_the_identity() {
    assert_eq!(
        Lut::<u8>::gain_offset(1.0, 0.0).as_slice(),
        Lut::<u8>::identity().as_slice()
    );
}

#[test]
fn gain_offset_saturates_at_both_ends() {
    // Estouro para cima: ganho 2 satura de 128 em diante.
    let up = ramp_levels(&Lut::<u8>::gain_offset(2.0, 0.0));
    assert_eq!(up[127], 254);
    assert!(up[128..].iter().all(|&v| v == 255));
    // Estouro para baixo: offset −300 zera tudo.
    assert!(ramp_levels(&Lut::<u8>::gain_offset(1.0, -300.0))
        .iter()
        .all(|&v| v == 0));
    // Offset +300 enche tudo.
    assert!(ramp_levels(&Lut::<u8>::gain_offset(1.0, 300.0))
        .iter()
        .all(|&v| v == 255));
    // Ganho negativo inverte e satura em 0 onde ficaria negativo.
    let flipped = ramp_levels(&Lut::<u8>::gain_offset(-1.0, 255.0));
    assert_eq!(flipped[0], 255);
    assert_eq!(flipped[255], 0);
}

#[test]
fn gain_offset_16_bit_saturates_at_65535() {
    let table = Lut::<u16>::gain_offset(2.0, 0.0);
    assert_eq!(table.as_slice()[32_767], 65_534);
    assert_eq!(table.as_slice()[32_768], 65_535);
    assert_eq!(table.as_slice()[65_535], 65_535);
    assert_eq!(
        Lut::<u16>::gain_offset(1.0, -70_000.0).as_slice()[65_535],
        0
    );
}

proptest! {
    #[test]
    fn gain_offset_never_leaves_the_range_and_follows_the_formula(
        gain in -4.0f64..4.0,
        offset in -400.0f64..400.0,
    ) {
        let table = Lut::<u8>::gain_offset(gain, offset);
        for (v, &out) in (0..=255u8).zip(table.as_slice()) {
            let expect = (gain * f64::from(v) + offset).round().clamp(0.0, 255.0);
            prop_assert!((f64::from(out) - expect).abs() < 0.5, "v = {}", v);
        }
    }
}

// ── Per canal, ROI e erros ─────────────────────────────────────────────────────

#[test]
fn per_channel_tables_apply_to_their_own_channel() {
    let image = Image::filled(3, 2, Rgb8::new(10, 20, 30)).unwrap();
    let tables = [
        Lut::<u8>::identity(),
        Lut::<u8>::invert(),
        Lut::<u8>::gain_offset(0.0, 7.0),
    ];
    let out = apply_lut_per_channel(image.view(), &tables).unwrap();
    assert!(out.as_pixels().iter().all(|p| p.0 == [10, 235, 7]));
}

#[test]
fn per_channel_rejects_a_wrong_number_of_tables() {
    let image = Image::filled(2, 2, Rgb8::new(1, 2, 3)).unwrap();
    let two = [Lut::<u8>::identity(), Lut::<u8>::identity()];
    let err = apply_lut_per_channel(image.view(), &two).unwrap_err();
    assert!(
        matches!(
            err,
            Error::ChannelMismatch {
                expected: 3,
                actual: 2
            }
        ),
        "{err:?}"
    );
}

#[test]
fn a_single_table_applies_to_every_channel() {
    let image = Image::filled(2, 2, Rgb8::new(0, 100, 255)).unwrap();
    let out = apply_lut(image.view(), &Lut::<u8>::invert()).unwrap();
    assert!(out.as_pixels().iter().all(|p| p.0 == [255, 155, 0]));
}

#[test]
fn a_strided_roi_is_processed_like_its_copy() {
    let image = perceptor_testkit::patterns::noise::<L8>(20, 12, 5);
    let region = Rect::new(3, 2, 9, 7);
    let table = Lut::<u8>::gamma(0.7);

    let from_view = apply_lut(image.roi(region).unwrap(), &table).unwrap();
    let from_copy = apply_lut(
        image.roi(region).unwrap().to_image().unwrap().view(),
        &table,
    )
    .unwrap();

    assert_images_eq!(from_view, from_copy);
}
