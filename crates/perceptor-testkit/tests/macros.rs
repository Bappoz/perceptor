//! As macros expandem corretamente fora da crate (`$crate` e `CARGO_MANIFEST_DIR`).

use perceptor_core::{Image, Rgb8, L8};
use perceptor_testkit::{assert_golden, assert_images_eq, patterns};

#[test]
fn assert_images_eq_works_from_a_downstream_crate() {
    let a = patterns::checkerboard::<L8>(8, 8, 2);
    assert_images_eq!(a, a.clone());
    let b = Image::filled(8, 8, L8::new(1)).unwrap();
    assert_images_eq!(b, b.clone(), tolerance = 0.0);
}

#[test]
fn assert_golden_reads_tests_golden_of_the_calling_crate() {
    // Regenerar: PERCEPTOR_UPDATE_GOLDEN=1 cargo test -p perceptor-testkit
    assert_golden!(patterns::noise::<Rgb8>(8, 8, 42), "noise_rgb_8x8.png");
    assert_golden!(
        patterns::gradient::<L8>(16, 4),
        "gradient_l8_16x4.png",
        tolerance = 0.0
    );
}
