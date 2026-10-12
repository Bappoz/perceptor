//! Conversão YUV → RGB: valores de referência, layouts e erros.
// `y`, `u`, `v`, `uv`, `cb`, `cr` são os nomes dos planos e canais do domínio.
#![allow(clippy::similar_names, clippy::many_single_char_names)]

use perceptor_core::{Error, Image, ImageView, Rgb8, L8};
use perceptor_imgproc::yuv::{
    i420_to_rgb, nv12_to_rgb, reference, yuyv_to_rgb, YuvMatrix, YuvRange, YuvSpec,
};
use perceptor_testkit::assert_images_eq;
use proptest::prelude::*;

const SPECS: [YuvSpec; 4] = [
    YuvSpec::BT601_LIMITED,
    YuvSpec::BT601_FULL,
    YuvSpec::BT709_LIMITED,
    YuvSpec::BT709_FULL,
];

fn spec() -> impl Strategy<Value = YuvSpec> {
    prop::sample::select(SPECS.to_vec())
}

/// Codificador independente, em `f64`, só para montar entradas de teste.
fn encode(rgb: Rgb8, spec: YuvSpec) -> [u8; 3] {
    let (kr, kb) = match spec.matrix {
        YuvMatrix::Bt601 => (0.299, 0.114),
        YuvMatrix::Bt709 => (0.2126, 0.0722),
    };
    let (r, g, b) = (f64::from(rgb.r()), f64::from(rgb.g()), f64::from(rgb.b()));
    let y = kr * r + (1.0 - kr - kb) * g + kb * b;
    let cb = (b - y) / (2.0 * (1.0 - kb));
    let cr = (r - y) / (2.0 * (1.0 - kr));
    let (y, cb, cr) = match spec.range {
        YuvRange::Full => (y, cb + 128.0, cr + 128.0),
        YuvRange::Limited => (
            16.0 + y * 219.0 / 255.0,
            128.0 + cb * 224.0 / 255.0,
            128.0 + cr * 224.0 / 255.0,
        ),
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let q = |v: f64| v.round().clamp(0.0, 255.0) as u8;
    [q(y), q(cb), q(cr)]
}

fn plane(width: usize, height: usize, bytes: &[u8]) -> Image<L8> {
    Image::from_subpixels(width, height, bytes).unwrap()
}

/// Os três layouts de uma imagem `w × h` com uma amostra YUV por bloco 2×2.
struct Layouts {
    y: Image<L8>,
    u: Image<L8>,
    v: Image<L8>,
    uv: Image<L8>,
    yuyv: Image<L8>,
}

fn layouts(blocks_w: usize, blocks_h: usize, yuv: &[[u8; 3]]) -> Layouts {
    let (w, h) = (blocks_w * 2, blocks_h * 2);
    let at = |x: usize, y: usize| yuv[(y / 2) * blocks_w + x / 2];
    let mut y = Vec::new();
    let mut yuyv = Vec::new();
    for row in 0..h {
        for col in 0..w {
            let [luma, cb, cr] = at(col, row);
            y.push(luma);
            yuyv.extend([luma, if col % 2 == 0 { cb } else { cr }]);
        }
    }
    let u: Vec<u8> = yuv.iter().map(|s| s[1]).collect();
    let v: Vec<u8> = yuv.iter().map(|s| s[2]).collect();
    let uv: Vec<u8> = yuv.iter().flat_map(|s| [s[1], s[2]]).collect();
    Layouts {
        y: plane(w, h, &y),
        u: plane(blocks_w, blocks_h, &u),
        v: plane(blocks_w, blocks_h, &v),
        uv: plane(w, blocks_h, &uv),
        yuyv: plane(w * 2, h, &yuyv),
    }
}

fn decode_one(yuv: [u8; 3], spec: YuvSpec) -> Rgb8 {
    let l = layouts(1, 1, &[yuv]);
    i420_to_rgb(l.y.view(), l.u.view(), l.v.view(), spec)
        .unwrap()
        .as_pixels()[0]
}

// ── Valores de referência ──────────────────────────────────────────────────────

#[test]
fn limited_range_black_and_white() {
    for spec in [YuvSpec::BT601_LIMITED, YuvSpec::BT709_LIMITED] {
        assert_eq!(decode_one([16, 128, 128], spec), Rgb8::new(0, 0, 0));
        assert_eq!(decode_one([235, 128, 128], spec), Rgb8::new(255, 255, 255));
        // Fora da faixa nominal satura em vez de dar a volta.
        assert_eq!(decode_one([0, 128, 128], spec), Rgb8::new(0, 0, 0));
        assert_eq!(decode_one([255, 128, 128], spec), Rgb8::new(255, 255, 255));
    }
}

#[test]
fn full_range_grays_are_the_luma() {
    for spec in [YuvSpec::BT601_FULL, YuvSpec::BT709_FULL] {
        for v in [0u8, 1, 127, 128, 200, 255] {
            assert_eq!(decode_one([v, 128, 128], spec), Rgb8::new(v, v, v));
        }
    }
}

#[test]
fn bt601_limited_primaries() {
    // Tripletos clássicos das barras de cor em BT.601 de faixa limitada. São
    // codificações arredondadas, então a volta exata não é a primária pura:
    // em `f64`, (81, 90, 240) dá (254,44; −0,48; −0,97).
    let spec = YuvSpec::BT601_LIMITED;
    let cases = [
        ([81, 90, 240], [254, 0, 0]),
        ([145, 54, 34], [0, 255, 1]),
        ([41, 240, 110], [0, 0, 255]),
    ];
    for (yuv, expected) in cases {
        assert_eq!(
            reference::yuv_to_rgb(yuv[0], yuv[1], yuv[2], spec),
            expected
        );
        let fast = decode_one(yuv, spec);
        assert_eq!([fast.r(), fast.g(), fast.b()], expected, "{yuv:?}");
    }
}

#[test]
fn default_spec_is_bt601_limited() {
    assert_eq!(YuvSpec::default(), YuvSpec::BT601_LIMITED);
}

#[test]
fn matrix_and_range_change_the_result() {
    let yuv = [120, 90, 200];
    let results: Vec<Rgb8> = SPECS.iter().map(|s| decode_one(yuv, *s)).collect();
    for i in 0..results.len() {
        for j in i + 1..results.len() {
            assert_ne!(results[i], results[j], "{:?} e {:?}", SPECS[i], SPECS[j]);
        }
    }
}

proptest! {
    #[test]
    fn stays_within_one_level_of_the_f64_reference(
        yuv in any::<[u8; 3]>(),
        spec in spec(),
    ) {
        let fast = decode_one(yuv, spec);
        let [r, g, b] = reference::yuv_to_rgb(yuv[0], yuv[1], yuv[2], spec);
        prop_assert!(fast.r().abs_diff(r) <= 1 && fast.g().abs_diff(g) <= 1 && fast.b().abs_diff(b) <= 1,
            "{fast:?} vs {:?}", [r, g, b]);
    }

    #[test]
    fn rgb_survives_encoding_and_decoding(rgb in any::<[u8; 3]>(), spec in spec()) {
        let rgb = Rgb8::new(rgb[0], rgb[1], rgb[2]);
        let back = decode_one(encode(rgb, spec), spec);
        // A quantização do YUV em 8 bits custa alguns níveis, mais na faixa limitada.
        let tolerance = 4;
        prop_assert!(
            rgb.r().abs_diff(back.r()) <= tolerance
                && rgb.g().abs_diff(back.g()) <= tolerance
                && rgb.b().abs_diff(back.b()) <= tolerance,
            "{rgb:?} -> {back:?}"
        );
    }

    #[test]
    fn the_three_layouts_decode_to_the_same_image(
        (blocks_w, blocks_h, samples) in (1usize..=6, 1usize..=6)
            .prop_flat_map(|(w, h)| (Just(w), Just(h), prop::collection::vec(any::<[u8; 3]>(), w * h))),
        spec in spec(),
    ) {
        let l = layouts(blocks_w, blocks_h, &samples);
        let planar = i420_to_rgb(l.y.view(), l.u.view(), l.v.view(), spec).unwrap();
        prop_assert_eq!(planar.dimensions(), (blocks_w * 2, blocks_h * 2));

        assert_images_eq!(nv12_to_rgb(l.y.view(), l.uv.view(), spec).unwrap(), planar);
        assert_images_eq!(yuyv_to_rgb(l.yuyv.view(), spec).unwrap(), planar);

        // Cada bloco 2×2 tem uma cor só, igual à da amostra.
        for (index, sample) in samples.iter().enumerate() {
            let (bx, by) = (index % blocks_w * 2, index / blocks_w * 2);
            let expected = decode_one(*sample, spec);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                prop_assert_eq!(planar.row(by + dy)[bx + dx], expected);
            }
        }
    }
}

// ── Layouts ────────────────────────────────────────────────────────────────────

#[test]
fn chroma_is_shared_by_the_block_and_luma_is_per_pixel() {
    // 2×2, um só par de croma (neutro); quatro lumas diferentes.
    let y = plane(2, 2, &[16, 235, 126, 180]);
    let u = plane(1, 1, &[128]);
    let v = plane(1, 1, &[128]);
    let rgb = i420_to_rgb(y.view(), u.view(), v.view(), YuvSpec::BT601_LIMITED).unwrap();
    let lumas: Vec<u8> = rgb.as_pixels().iter().map(|p| p.g()).collect();
    assert_eq!(lumas, [0, 255, 128, 191]);
    assert!(rgb
        .as_pixels()
        .iter()
        .all(|p| p.r() == p.g() && p.g() == p.b()));
}

#[test]
fn yuyv_shares_chroma_horizontally_only() {
    // Linha 0 com croma vermelho, linha 1 neutra: 4:2:2 não mistura linhas.
    let packed = plane(4, 2, &[81, 90, 81, 240, 126, 128, 126, 128]);
    let rgb = yuyv_to_rgb(packed.view(), YuvSpec::BT601_LIMITED).unwrap();
    assert_eq!(rgb.dimensions(), (2, 2));
    assert_eq!(rgb.row(0), [Rgb8::new(254, 0, 0); 2]);
    assert_eq!(rgb.row(1), [Rgb8::new(128, 128, 128); 2]);
}

#[test]
fn planes_are_read_through_their_own_stride() {
    // Um único buffer NV12 de 4×2 com 2 bytes de preenchimento por linha:
    // as visões apontam para dentro dele, sem cópia.
    let stride = 6;
    #[rustfmt::skip]
    let buffer = [
        16, 16, 235, 235, 99, 99,
        16, 16, 235, 235, 99, 99,
        128, 128, 128, 128, 99, 99,
    ];
    let y = ImageView::<L8>::new(&buffer[..2 * stride], 4, 2, stride).unwrap();
    let uv = ImageView::<L8>::new(&buffer[2 * stride..], 4, 1, stride).unwrap();

    let rgb = nv12_to_rgb(y, uv, YuvSpec::BT601_LIMITED).unwrap();

    let black = Rgb8::new(0, 0, 0);
    let white = Rgb8::new(255, 255, 255);
    assert_eq!(rgb.row(0), [black, black, white, white]);
    assert_eq!(rgb.row(1), [black, black, white, white]);
}

// ── Erros ──────────────────────────────────────────────────────────────────────

#[test]
fn odd_dimensions_are_an_explicit_error() {
    let chroma = plane(1, 1, &[128]);
    let spec = YuvSpec::default();
    for (w, h) in [(3, 2), (2, 3), (3, 3), (1, 1)] {
        let y = Image::<L8>::new(w, h).unwrap();
        let err = i420_to_rgb(y.view(), chroma.view(), chroma.view(), spec).unwrap_err();
        assert!(
            matches!(err, Error::OddDimensions { width, height } if (width, height) == (w, h)),
            "{w}×{h}: {err:?}"
        );
        let uv = Image::<L8>::new(2, 1).unwrap();
        assert!(matches!(
            nv12_to_rgb(y.view(), uv.view(), spec),
            Err(Error::OddDimensions { .. })
        ));
    }
}

#[test]
fn yuyv_with_odd_width_is_an_explicit_error() {
    let spec = YuvSpec::default();
    // 6 bytes por linha = 3 pixels.
    let three_pixels = Image::<L8>::new(6, 2).unwrap();
    assert!(matches!(
        yuyv_to_rgb(three_pixels.view(), spec),
        Err(Error::OddDimensions {
            width: 3,
            height: 2
        })
    ));
    // 5 bytes por linha não são um número inteiro de pixels.
    let ragged = Image::<L8>::new(5, 2).unwrap();
    assert!(matches!(
        yuyv_to_rgb(ragged.view(), spec),
        Err(Error::BufferLength {
            len: 5,
            channels: 2
        })
    ));
    // Altura ímpar é válida em 4:2:2.
    let tall = Image::<L8>::new(4, 3).unwrap();
    assert_eq!(yuyv_to_rgb(tall.view(), spec).unwrap().dimensions(), (2, 3));
}

#[test]
fn chroma_planes_of_the_wrong_size_are_rejected() {
    let spec = YuvSpec::default();
    let y = Image::<L8>::new(4, 4).unwrap();
    let good = Image::<L8>::new(2, 2).unwrap();
    let bad = Image::<L8>::new(4, 4).unwrap();

    assert!(matches!(
        i420_to_rgb(y.view(), bad.view(), good.view(), spec),
        Err(Error::ShapeMismatch {
            expected: (2, 2),
            actual: (4, 4)
        })
    ));
    assert!(matches!(
        i420_to_rgb(y.view(), good.view(), bad.view(), spec),
        Err(Error::ShapeMismatch {
            expected: (2, 2),
            actual: (4, 4)
        })
    ));
    // NV12: o plano UV tem a largura do Y em bytes e metade da altura.
    assert!(matches!(
        nv12_to_rgb(y.view(), good.view(), spec),
        Err(Error::ShapeMismatch {
            expected: (4, 2),
            actual: (2, 2)
        })
    ));
}
