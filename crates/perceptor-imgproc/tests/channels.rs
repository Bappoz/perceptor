//! Conversões de layout de canais: propriedades, casos à mão e goldens.
//!
//! Regenerar os goldens: `PERCEPTOR_UPDATE_GOLDEN=1 cargo test -p perceptor-imgproc --test channels`

use perceptor_core::{
    Bgr8, Error, Image, ImageView, La8, Pixel, Rect, Rgb8, RgbF32, Rgba8, L8, LF32,
};
use perceptor_imgproc::channels::{
    add_alpha, bgr_to_rgb, bgr_to_rgba, drop_alpha, extract_channel, merge_channels, rgb_to_bgr,
    rgba_to_bgr, split_channels, swap_channels,
};
use perceptor_testkit::strategies::any_image;
use perceptor_testkit::{assert_golden, assert_images_eq};
use proptest::prelude::*;

/// Barras de cor clássicas, 4 pixels cada, mais escuras na metade de baixo.
fn bars() -> Image<Rgb8> {
    const COLORS: [(u8, u8, u8); 8] = [
        (255, 255, 255),
        (255, 255, 0),
        (0, 255, 255),
        (0, 255, 0),
        (255, 0, 255),
        (255, 0, 0),
        (0, 0, 255),
        (0, 0, 0),
    ];
    Image::from_fn(32, 16, |x, y| {
        let (r, g, b) = COLORS[x / 4];
        let dim = |v: u8| if y < 8 { v } else { v / 2 };
        Rgb8::new(dim(r), dim(g), dim(b))
    })
    .unwrap()
}

/// Reinterpreta a memória de uma imagem BGR como RGB, para compará-la a um
/// PNG: o golden registra a ordem dos bytes, não a cor.
fn memory_as_rgb(image: &Image<Bgr8>) -> Image<Rgb8> {
    Image::from_subpixels(image.width(), image.height(), image.as_subpixels()).unwrap()
}

fn views<P: Pixel>(planes: &[Image<P>]) -> Vec<ImageView<'_, P>> {
    planes.iter().map(Image::view).collect()
}

// ── Casos à mão ────────────────────────────────────────────────────────────────

#[test]
fn rgb_to_bgr_reverses_the_bytes_and_keeps_the_color() {
    let rgb = Image::from_pixels(2, 1, &[Rgb8::new(1, 2, 3), Rgb8::new(40, 50, 60)]).unwrap();

    let bgr = rgb_to_bgr(rgb.view()).unwrap();

    assert_eq!(bgr.as_subpixels(), [3, 2, 1, 60, 50, 40]);
    assert_eq!(
        (bgr.row(0)[0].r(), bgr.row(0)[0].g(), bgr.row(0)[0].b()),
        (1, 2, 3)
    );
    assert_eq!(bgr_to_rgb(bgr.view()).unwrap().as_pixels(), rgb.as_pixels());
}

#[test]
fn add_alpha_appends_a_constant_channel() {
    let rgb = Image::filled(2, 2, Rgb8::new(1, 2, 3)).unwrap();
    let rgba = add_alpha(rgb.view(), 200).unwrap();
    assert_eq!(rgba.as_pixels(), [Rgba8::new(1, 2, 3, 200); 4]);

    let gray = Image::filled(2, 2, L8::new(9)).unwrap();
    let la = add_alpha(gray.view(), 7).unwrap();
    assert_eq!(la.as_pixels(), [La8::new(9, 7); 4]);
}

#[test]
fn drop_alpha_discards_alpha_without_blending() {
    let rgba = Image::from_pixels(2, 1, &[Rgba8::new(10, 20, 30, 0), Rgba8::new(1, 2, 3, 255)]);
    let rgb = drop_alpha(rgba.unwrap().view()).unwrap();
    assert_eq!(rgb.as_pixels(), [Rgb8::new(10, 20, 30), Rgb8::new(1, 2, 3)]);

    let la = Image::filled(1, 1, La8::new(77, 3)).unwrap();
    assert_eq!(drop_alpha(la.view()).unwrap().as_pixels(), [L8::new(77)]);
}

#[test]
fn bgr_and_rgba_convert_directly() {
    let bgr = Image::filled(1, 1, Bgr8::new(1, 2, 3)).unwrap();
    let rgba = bgr_to_rgba(bgr.view(), 9).unwrap();
    assert_eq!(rgba.as_pixels(), [Rgba8::new(1, 2, 3, 9)]);
    assert_eq!(rgba_to_bgr(rgba.view()).unwrap().as_subpixels(), [3, 2, 1]);
}

#[test]
fn swap_channels_works_in_place_on_any_format() {
    let mut rgba = Image::filled(2, 1, Rgba8::new(1, 2, 3, 4)).unwrap();
    swap_channels(rgba.view_mut(), 0, 3).unwrap();
    assert_eq!(rgba.as_subpixels(), [4, 2, 3, 1, 4, 2, 3, 1]);

    let mut float = Image::filled(1, 1, RgbF32::new(0.1, 0.2, 0.3)).unwrap();
    swap_channels(float.view_mut(), 1, 2).unwrap();
    assert_eq!(float.as_subpixels(), [0.1, 0.3, 0.2]);

    let before = rgba.clone();
    swap_channels(rgba.view_mut(), 2, 2).unwrap();
    assert_images_eq!(rgba, before);
}

#[test]
fn swap_channels_rejects_an_index_out_of_range() {
    let mut rgb = Image::filled(1, 1, Rgb8::new(1, 2, 3)).unwrap();

    let err = swap_channels(rgb.view_mut(), 0, 3).unwrap_err();

    assert!(matches!(
        err,
        Error::ChannelOutOfRange {
            index: 3,
            channels: 3
        }
    ));
    assert_eq!(rgb.as_subpixels(), [1, 2, 3], "nada muda quando falha");
}

#[test]
fn extract_channel_follows_memory_order() {
    let bgr = Image::filled(2, 2, Bgr8::new(10, 20, 30)).unwrap();
    // Memória `[B, G, R]`: o canal 0 é o azul.
    assert_eq!(
        extract_channel(bgr.view(), 0).unwrap().as_pixels(),
        [L8::new(30); 4]
    );
    assert_eq!(
        extract_channel(bgr.view(), 2).unwrap().as_pixels(),
        [L8::new(10); 4]
    );
    assert!(matches!(
        extract_channel(bgr.view(), 3),
        Err(Error::ChannelOutOfRange {
            index: 3,
            channels: 3
        })
    ));
}

#[test]
fn split_gives_one_plane_per_channel() {
    let rgba = Image::filled(3, 2, Rgba8::new(1, 2, 3, 4)).unwrap();
    let planes = split_channels(rgba.view()).unwrap();
    assert_eq!(planes.len(), 4);
    for (plane, value) in planes.iter().zip(1u8..) {
        assert_eq!(plane.dimensions(), (3, 2));
        assert!(plane.as_pixels().iter().all(|p| p.luma() == value));
    }

    let float = Image::filled(1, 1, RgbF32::new(0.25, 0.5, 0.75)).unwrap();
    let planes: Vec<Image<LF32>> = split_channels(float.view()).unwrap();
    assert_eq!(planes[2].as_pixels(), [LF32::new(0.75)]);
}

#[test]
fn merge_rejects_the_wrong_number_of_planes() {
    let plane = Image::<L8>::new(2, 2).unwrap();
    let two = [plane.view(), plane.view()];

    let err = merge_channels::<Rgb8>(&two).unwrap_err();
    assert!(matches!(
        err,
        Error::ChannelMismatch {
            expected: 3,
            actual: 2
        }
    ));
    assert!(matches!(
        merge_channels::<Rgb8>(&[]),
        Err(Error::ChannelMismatch {
            expected: 3,
            actual: 0
        })
    ));
}

#[test]
fn merge_rejects_planes_of_different_sizes() {
    let a = Image::<L8>::new(2, 2).unwrap();
    let b = Image::<L8>::new(2, 3).unwrap();

    let err = merge_channels::<Rgb8>(&[a.view(), a.view(), b.view()]).unwrap_err();

    assert!(matches!(
        err,
        Error::ShapeMismatch {
            expected: (2, 2),
            actual: (2, 3)
        }
    ));
}

// ── Round-trips ────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn rgb_bgr_round_trip(image in any_image::<Rgb8>(1..=16, 1..=16)) {
        let back = bgr_to_rgb(rgb_to_bgr(image.view()).unwrap().view()).unwrap();
        assert_images_eq!(back, image);
    }

    #[test]
    fn add_then_drop_alpha_is_identity(
        rgb in any_image::<Rgb8>(1..=16, 1..=16),
        gray in any_image::<L8>(1..=16, 1..=16),
        alpha in any::<u8>(),
    ) {
        let back = drop_alpha(add_alpha(rgb.view(), alpha).unwrap().view()).unwrap();
        assert_images_eq!(back, rgb);
        let back = drop_alpha(add_alpha(gray.view(), alpha).unwrap().view()).unwrap();
        assert_images_eq!(back, gray);
    }

    #[test]
    fn bgr_rgba_round_trip(image in any_image::<Bgr8>(1..=16, 1..=16), alpha in any::<u8>()) {
        let rgba = bgr_to_rgba(image.view(), alpha).unwrap();
        prop_assert!(rgba.as_pixels().iter().all(|p| p.a() == alpha));
        assert_images_eq!(rgba_to_bgr(rgba.view()).unwrap(), image);
    }

    #[test]
    fn split_then_merge_is_identity(
        rgb in any_image::<Rgb8>(1..=12, 1..=12),
        bgr in any_image::<Bgr8>(1..=12, 1..=12),
        rgba in any_image::<Rgba8>(1..=12, 1..=12),
        la in any_image::<La8>(1..=12, 1..=12),
        float in any_image::<RgbF32>(1..=12, 1..=12),
    ) {
        assert_images_eq!(merge_channels::<Rgb8>(&views(&split_channels(rgb.view()).unwrap())).unwrap(), rgb);
        assert_images_eq!(merge_channels::<Bgr8>(&views(&split_channels(bgr.view()).unwrap())).unwrap(), bgr);
        assert_images_eq!(merge_channels::<Rgba8>(&views(&split_channels(rgba.view()).unwrap())).unwrap(), rgba);
        assert_images_eq!(merge_channels::<La8>(&views(&split_channels(la.view()).unwrap())).unwrap(), la);
        assert_images_eq!(merge_channels::<RgbF32>(&views(&split_channels(float.view()).unwrap())).unwrap(), float);
    }

    #[test]
    fn extract_matches_the_corresponding_split_plane(
        image in any_image::<Rgba8>(1..=12, 1..=12),
        index in 0usize..4,
    ) {
        let planes = split_channels(image.view()).unwrap();
        assert_images_eq!(extract_channel(image.view(), index).unwrap(), planes[index]);
    }

    #[test]
    fn swapping_twice_is_identity_and_order_does_not_matter(
        image in any_image::<Rgba8>(1..=12, 1..=12),
        a in 0usize..4,
        b in 0usize..4,
    ) {
        let mut once = image.clone();
        swap_channels(once.view_mut(), a, b).unwrap();
        let mut reversed = image.clone();
        swap_channels(reversed.view_mut(), b, a).unwrap();
        assert_images_eq!(once, reversed);

        swap_channels(once.view_mut(), a, b).unwrap();
        assert_images_eq!(once, image);
    }

    #[test]
    fn swapping_red_and_blue_in_place_matches_rgb_to_bgr(
        image in any_image::<Rgb8>(1..=16, 1..=16),
    ) {
        let converted = rgb_to_bgr(image.view()).unwrap();
        let mut swapped = image.clone();
        swap_channels(swapped.view_mut(), 0, 2).unwrap();
        prop_assert_eq!(swapped.as_subpixels(), converted.as_subpixels());
    }

    #[test]
    fn a_roi_converts_like_the_same_region_of_the_whole(
        image in any_image::<Rgb8>(4..=16, 4..=16),
    ) {
        let rect = Rect::new(1, 1, image.width() - 2, image.height() - 2);
        let whole = rgb_to_bgr(image.view()).unwrap();
        let part = rgb_to_bgr(image.roi(rect).unwrap()).unwrap();
        for (got, want) in part.rows().zip(whole.roi(rect).unwrap().rows()) {
            prop_assert_eq!(got, want);
        }

        // In-place em um recorte: só o recorte muda.
        let mut target = image.clone();
        swap_channels(target.roi_mut(rect).unwrap(), 0, 2).unwrap();
        prop_assert_eq!(target.row(0), image.row(0));
        let inner = target.roi(rect).unwrap();
        prop_assert_eq!(
            Rgb8::as_subpixels(inner.row(0)),
            Bgr8::as_subpixels(part.row(0))
        );
    }
}

// ── Goldens ────────────────────────────────────────────────────────────────────

#[test]
fn golden_rgb_to_bgr() {
    let bgr = rgb_to_bgr(bars().view()).unwrap();
    assert_golden!(memory_as_rgb(&bgr), "channels_bars_bgr_memory.png");
}

#[test]
fn golden_bgr_to_rgb() {
    let bgr = rgb_to_bgr(bars().view()).unwrap();
    assert_golden!(bgr_to_rgb(bgr.view()).unwrap(), "channels_bars_rgb.png");
}

#[test]
fn golden_swap_channels() {
    let mut image = bars();
    swap_channels(image.view_mut(), 0, 1).unwrap();
    assert_golden!(image, "channels_bars_swap_rg.png");
}

#[test]
fn golden_add_alpha() {
    assert_golden!(
        add_alpha(bars().view(), 128).unwrap(),
        "channels_bars_rgba_a128.png"
    );
    let gray = extract_channel(bars().view(), 1).unwrap();
    assert_golden!(
        add_alpha(gray.view(), 64).unwrap(),
        "channels_bars_green_la_a64.png"
    );
}

#[test]
fn golden_drop_alpha() {
    let rgba = add_alpha(bars().view(), 128).unwrap();
    assert_golden!(drop_alpha(rgba.view()).unwrap(), "channels_bars_rgb.png");
}

#[test]
fn golden_bgr_rgba() {
    let bgr = rgb_to_bgr(bars().view()).unwrap();
    let rgba = bgr_to_rgba(bgr.view(), 128).unwrap();
    assert_golden!(rgba, "channels_bars_rgba_a128.png");
    let back = rgba_to_bgr(rgba.view()).unwrap();
    assert_golden!(memory_as_rgb(&back), "channels_bars_bgr_memory.png");
}

#[test]
fn golden_split_and_extract() {
    let planes = split_channels(bars().view()).unwrap();
    assert_golden!(planes[0], "channels_bars_plane_r.png");
    assert_golden!(planes[1], "channels_bars_plane_g.png");
    assert_golden!(planes[2], "channels_bars_plane_b.png");
    assert_golden!(
        extract_channel(bars().view(), 2).unwrap(),
        "channels_bars_plane_b.png"
    );
}

#[test]
fn golden_merge() {
    let planes = split_channels(bars().view()).unwrap();
    let merged = merge_channels::<Rgb8>(&views(&planes)).unwrap();
    assert_golden!(merged, "channels_bars_rgb.png");
    // Planos em outra ordem: R e B trocados na memória.
    let reversed = [planes[2].view(), planes[1].view(), planes[0].view()];
    let swapped = merge_channels::<Rgb8>(&reversed).unwrap();
    assert_golden!(swapped, "channels_bars_bgr_memory.png");
}
