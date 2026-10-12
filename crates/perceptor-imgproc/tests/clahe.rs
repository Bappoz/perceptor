//! CLAHE: parâmetros, continuidade entre tiles e golden.
//!
//! Regenerar os goldens: `PERCEPTOR_UPDATE_GOLDEN=1 cargo test -p perceptor-imgproc --test clahe`

use perceptor_core::{Error, Image, Rect, L8};
use perceptor_imgproc::clahe::clahe;
use perceptor_imgproc::histogram::equalize;
use perceptor_testkit::assert_golden;
use perceptor_testkit::patterns::noise;
use perceptor_testkit::strategies::any_image;
use proptest::prelude::*;

fn levels(image: &Image<L8>) -> Vec<u8> {
    image.as_pixels().iter().map(|p| p.luma()).collect()
}

/// Maior salto entre dois pixels vizinhos na horizontal.
fn max_horizontal_jump(image: &Image<L8>) -> u8 {
    image
        .as_pixels()
        .chunks_exact(image.width())
        .flat_map(|row| row.windows(2).map(|w| w[0].luma().abs_diff(w[1].luma())))
        .max()
        .unwrap_or(0)
}

/// Rampa horizontal suave: 0 a 254 em `width` colunas.
fn ramp(width: usize, height: usize) -> Image<L8> {
    Image::from_fn(width, height, |x, _| {
        L8::new(u8::try_from(x * 254 / (width - 1)).unwrap())
    })
    .unwrap()
}

/// Cena de baixo contraste: fundo em degradê (90–130), um disco e barras.
fn dull_scene() -> Image<L8> {
    let signed = |v: usize| i32::try_from(v).unwrap();
    Image::from_fn(96, 64, |x, y| {
        let background = 90 + signed(x) * 40 / 95;
        let (dx, dy) = (signed(x) - 30, signed(y) - 32);
        let disc = dx * dx + dy * dy < 14 * 14;
        let bar = (60..90).contains(&x) && (y / 8) % 2 == 0;
        let level = background + if disc { 12 } else { 0 } - if bar { 10 } else { 0 };
        L8::new(u8::try_from(level.clamp(0, 255)).unwrap())
    })
    .unwrap()
}

// ── Parâmetros ─────────────────────────────────────────────────────────────────

fn invalid_name(err: &Error) -> Option<&'static str> {
    match err {
        Error::InvalidParameter { name, .. } => Some(name),
        _ => None,
    }
}

#[test]
fn rejects_an_empty_tile_grid() {
    let image = noise::<L8>(16, 16, 1);
    for tiles in [(0, 4), (4, 0), (0, 0)] {
        let err = clahe(image.view(), 2.0, tiles).unwrap_err();
        assert_eq!(invalid_name(&err), Some("tiles"), "{tiles:?}: {err:?}");
    }
}

#[test]
fn rejects_more_tiles_than_pixels() {
    let image = noise::<L8>(6, 4, 1);
    for tiles in [(7, 1), (1, 5), (8, 8)] {
        let err = clahe(image.view(), 2.0, tiles).unwrap_err();
        assert_eq!(invalid_name(&err), Some("tiles"), "{tiles:?}: {err:?}");
    }
    assert!(
        clahe(image.view(), 2.0, (6, 4)).is_ok(),
        "um pixel por tile cabe"
    );
}

#[test]
fn rejects_a_clip_limit_that_is_not_finite_and_positive() {
    let image = noise::<L8>(16, 16, 1);
    for clip in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let err = clahe(image.view(), clip, (2, 2)).unwrap_err();
        assert_eq!(invalid_name(&err), Some("clip_limit"), "{clip}: {err:?}");
    }
}

#[test]
fn keeps_the_input_dimensions() {
    let image = noise::<L8>(37, 23, 4);
    let out = clahe(image.view(), 2.0, (5, 3)).unwrap();
    assert_eq!((out.width(), out.height()), (37, 23));
}

proptest! {
    #[test]
    fn any_valid_grid_runs_and_keeps_the_size(
        (image, tiles) in any_image::<L8>(1..=24, 1..=24).prop_flat_map(|image| {
            let (w, h) = (image.width(), image.height());
            (Just(image), (1..=w, 1..=h))
        }),
        clip in 0.5f64..10.0,
    ) {
        let out = clahe(image.view(), clip, tiles).unwrap();
        prop_assert_eq!((out.width(), out.height()), (image.width(), image.height()));
    }
}

// ── Comportamento ──────────────────────────────────────────────────────────────

#[test]
fn a_higher_clip_limit_gives_more_contrast() {
    // Ruído estreito (100–120): mais limite deixa o contraste local crescer.
    let image = Image::from_fn(64, 64, |x, y| {
        L8::new(100 + u8::try_from((x * 7 + y * 13) % 21).unwrap())
    })
    .unwrap();
    let spread = |clip: f64| {
        let out = levels(&clahe(image.view(), clip, (4, 4)).unwrap());
        let (lo, hi) = (*out.iter().min().unwrap(), *out.iter().max().unwrap());
        i32::from(hi) - i32::from(lo)
    };
    assert!(
        spread(8.0) > spread(1.5),
        "{} vs {}",
        spread(8.0),
        spread(1.5)
    );
}

#[test]
fn tiles_are_blended_not_stitched() {
    // Equalizar cada tile sozinho dá um salto enorme na fronteira (o fim de
    // um tile vai a 255 e o começo do próximo a 0); o CLAHE interpola as
    // tabelas e a rampa continua suave.
    let image = ramp(128, 32);
    let tile_width = 32;

    let mut stitched = Image::<L8>::new(128, 32).unwrap();
    for tx in 0..4 {
        let region = Rect::new(tx * tile_width, 0, tile_width, 32);
        let tile = equalize(image.roi(region).unwrap()).unwrap();
        for y in 0..32 {
            for x in 0..tile_width {
                stitched.as_pixels_mut()[y * 128 + tx * tile_width + x] =
                    tile.as_pixels()[y * tile_width + x];
            }
        }
    }
    assert!(
        max_horizontal_jump(&stitched) > 200,
        "a referência sem mistura devia ter degraus de bloco: {}",
        max_horizontal_jump(&stitched)
    );

    let blended = clahe(image.view(), 2.0, (4, 1)).unwrap();
    assert!(
        max_horizontal_jump(&blended) <= 12,
        "salto de {} entre vizinhos",
        max_horizontal_jump(&blended)
    );
}

// ── Golden ─────────────────────────────────────────────────────────────────────

#[test]
fn golden_input_scene() {
    assert_golden!(dull_scene(), "clahe_scene_input.png");
}

#[test]
fn golden_clahe_on_a_dull_scene() {
    let out = clahe(dull_scene().view(), 3.0, (4, 4)).unwrap();
    assert_golden!(out, "clahe_scene_clip3_tiles4x4.png");
}
