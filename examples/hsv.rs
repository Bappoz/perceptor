//! RGB ↔ HSV: convenções de faixa e segmentação por matiz.
//!
//! ```text
//! cargo run --example hsv
//! ```

use perceptor::core::Result;
use perceptor::imgproc::hsv::{hsv_to_rgb, rgb_to_hsl, rgb_to_hsv, HSV8_ROUND_TRIP_TOLERANCE};
use perceptor::prelude::*;

fn main() -> Result<()> {
    // Um vermelho vivo, o mesmo vermelho na sombra, um verde e um cinza.
    let pixels = [
        Rgb8::new(230, 30, 20),
        Rgb8::new(90, 12, 8),
        Rgb8::new(20, 200, 40),
        Rgb8::new(128, 128, 128),
    ];
    let rgb = Image::from_pixels(4, 1, &pixels)?;

    // 8 bits: matiz em 0..180 (graus / 2), saturação e valor em 0..=255.
    let hsv = rgb_to_hsv(rgb.view())?;
    for (rgb, hsv) in pixels.iter().zip(hsv.as_pixels()) {
        println!("{rgb:?} -> H={:3} S={:3} V={:3}", hsv.h(), hsv.s(), hsv.v());
    }

    // Segmentar por cor: "é vermelho" vira uma faixa de matiz com saturação
    // mínima, e vale tanto para o tom vivo quanto para o da sombra. Em RGB
    // os dois não têm um limiar em comum.
    let is_red = |p: &Hsv8| (p.h() < 10 || p.h() >= 170) && p.s() > 100;
    let mask: Vec<bool> = hsv.as_pixels().iter().map(is_red).collect();
    assert_eq!(mask, [true, true, false, false]);

    // Cinza não tem matiz: H = 0 e S = 0.
    assert_eq!(hsv.row(0)[3], Hsv8::new(0, 0, 128));

    // A volta em 8 bits não é exata: a quantização custa alguns níveis.
    let back = hsv_to_rgb(hsv.view())?;
    for (a, b) in pixels.iter().zip(back.as_pixels()) {
        let error = a
            .r()
            .abs_diff(b.r())
            .max(a.g().abs_diff(b.g()))
            .max(a.b().abs_diff(b.b()));
        assert!(error <= HSV8_ROUND_TRIP_TOLERANCE);
    }

    // f32: matiz em graus, 0.0..360.0; o resto em 0.0..=1.0.
    let float = Image::filled(1, 1, RgbF32::new(0.0, 0.5, 1.0))?;
    let hsv = rgb_to_hsv(float.view())?.row(0)[0];
    let hsl = rgb_to_hsl(float.view())?.row(0)[0];
    println!("azul-celeste: HSV = {hsv:?}, HSL = {hsl:?}");
    assert!((hsv.h() - 210.0).abs() < 1e-3 && (hsl.l() - 0.5).abs() < 1e-6);
    Ok(())
}
