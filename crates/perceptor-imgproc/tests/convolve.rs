//! Convolução e correlação 2D de referência, com `BorderMode`.

// Os esperados em `f32` são inteiros pequenos, exatos em binário.
#![allow(clippy::float_cmp)]

use perceptor_core::{BorderMode, Error, Image, ImageView, Pixel, Rect, Rgb8, RgbF32, L8, LF32};
use perceptor_imgproc::convolve::{convolve2d, correlate2d, Kernel};
use perceptor_imgproc::sobel::apply_sobel;
use perceptor_testkit::assert_images_eq;
use perceptor_testkit::patterns::noise;
use perceptor_testkit::strategies::{any_image, small_image};
use proptest::prelude::*;

fn l8(width: usize, values: &[u8]) -> Image<L8> {
    Image::from_fn(width, values.len() / width, |x, y| {
        L8::new(values[y * width + x])
    })
    .unwrap()
}

fn l8_values(image: &Image<L8>) -> Vec<u8> {
    image.as_pixels().iter().map(|p| p.luma()).collect()
}

fn lf32_values(image: &Image<LF32>) -> Vec<f32> {
    image.as_pixels().iter().map(|p| p.luma()).collect()
}

/// Imagem `f32` com os mesmos valores numéricos (sem reescalar para `[0, 1]`).
fn raw_f32(image: &Image<L8>) -> Image<LF32> {
    Image::from_fn(image.width(), image.height(), |x, y| {
        LF32::new(f32::from(image.row(y)[x].luma()))
    })
    .unwrap()
}

/// Kernel com `1` no centro e `0` no resto.
fn impulse_kernel(width: usize, height: usize) -> Kernel {
    let mut weights = vec![0.0; width * height];
    weights[(height / 2) * width + width / 2] = 1.0;
    Kernel::new(width, height, weights).unwrap()
}

/// Correlação pela definição, pixel a pixel, com `get_bordered`: o oráculo.
fn naive_correlate(input: ImageView<'_, L8>, kernel: &Kernel, border: BorderMode<L8>) -> Vec<f64> {
    let signed = |v: usize| isize::try_from(v).unwrap();
    let (kw, kh) = (kernel.width(), kernel.height());
    let mut out = Vec::new();
    for y in 0..input.height() {
        for x in 0..input.width() {
            let mut acc = 0.0;
            for ky in 0..kh {
                for kx in 0..kw {
                    let weight = kernel.weights()[ky * kw + kx];
                    let pixel = input.get_bordered(
                        signed(x + kx) - signed(kw / 2),
                        signed(y + ky) - signed(kh / 2),
                        border,
                    );
                    acc += f64::from(weight) * f64::from(pixel.luma());
                }
            }
            out.push(acc);
        }
    }
    out
}

fn any_border() -> impl Strategy<Value = BorderMode<L8>> {
    prop_oneof![
        any::<u8>().prop_map(|v| BorderMode::Constant(L8::new(v))),
        Just(BorderMode::Replicate),
        Just(BorderMode::Reflect),
        Just(BorderMode::Reflect101),
        Just(BorderMode::Wrap),
    ]
}

/// Lados ímpares de 1 a 7.
fn odd_side() -> impl Strategy<Value = usize> {
    (0usize..4).prop_map(|n| 2 * n + 1)
}

/// Kernel de pesos inteiros em `[-4, 4]`: toda a conta fica exata em `f32`.
fn integer_kernel() -> impl Strategy<Value = Kernel> {
    (odd_side(), odd_side()).prop_flat_map(|(w, h)| {
        proptest::collection::vec(-4i8..=4, w * h).prop_map(move |weights| {
            Kernel::new(w, h, weights.into_iter().map(f32::from).collect()).unwrap()
        })
    })
}

fn float_kernel() -> impl Strategy<Value = Kernel> {
    (odd_side(), odd_side()).prop_flat_map(|(w, h)| {
        proptest::collection::vec(-1.0f32..=1.0, w * h)
            .prop_map(move |weights| Kernel::new(w, h, weights).unwrap())
    })
}

// ── Kernel ─────────────────────────────────────────────────────────────────────

#[test]
fn kernel_keeps_dimensions_and_row_major_weights() {
    let kernel = Kernel::from_rows([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]).unwrap();
    assert_eq!((kernel.width(), kernel.height()), (3, 3));
    assert_eq!(
        kernel.weights(),
        [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]
    );

    let row = Kernel::from_rows([[1.0, 0.0, -1.0]]).unwrap();
    assert_eq!((row.width(), row.height()), (3, 1));
}

#[test]
fn flipped_rotates_the_kernel_by_180_degrees() {
    let kernel = Kernel::from_rows([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]).unwrap();
    let flipped = kernel.flipped();
    assert_eq!(
        flipped.weights(),
        [9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0]
    );
    assert_eq!(flipped.flipped(), kernel);
}

#[test]
fn even_or_empty_sides_are_rejected() {
    for (width, height, name) in [(2, 3, "width"), (3, 4, "height"), (0, 1, "width")] {
        let err = Kernel::new(width, height, vec![0.0; width * height]).unwrap_err();
        assert!(
            matches!(err, Error::InvalidParameter { name: n, .. } if n == name),
            "{width}×{height}: {err:?}"
        );
    }
    assert!(Kernel::from_rows([[1.0, 1.0], [1.0, 1.0]]).is_err());
}

#[test]
fn weights_must_match_the_dimensions_and_be_finite() {
    let short = Kernel::new(3, 3, vec![0.0; 8]).unwrap_err();
    assert!(matches!(
        short,
        Error::InvalidParameter {
            name: "weights",
            ..
        }
    ));
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let err = Kernel::new(1, 1, vec![bad]).unwrap_err();
        assert!(matches!(
            err,
            Error::InvalidParameter {
                name: "weights",
                ..
            }
        ));
    }
}

// ── Valores à mão ──────────────────────────────────────────────────────────────

#[test]
fn correlation_reads_the_neighbour_under_the_weight_and_convolution_the_opposite() {
    // Peso 1 à direita do centro.
    let kernel = Kernel::from_rows([[0.0, 0.0, 1.0]]).unwrap();
    let image = l8(4, &[10, 20, 30, 40]);
    let border = BorderMode::Constant(L8::new(0));

    let correlated: Image<L8> = correlate2d(image.view(), &kernel, border).unwrap();
    assert_eq!(
        l8_values(&correlated),
        [20, 30, 40, 0],
        "saída(x) = entrada(x + 1)"
    );

    let convolved: Image<L8> = convolve2d(image.view(), &kernel, border).unwrap();
    assert_eq!(
        l8_values(&convolved),
        [0, 10, 20, 30],
        "saída(x) = entrada(x − 1)"
    );
}

#[test]
fn each_border_mode_supplies_the_missing_neighbour() {
    // saída(x) = entrada(x − 2): as duas primeiras colunas vêm da borda.
    let kernel = Kernel::from_rows([[1.0, 0.0, 0.0, 0.0, 0.0]]).unwrap();
    let image = l8(4, &[10, 20, 30, 40]);
    let cases = [
        (BorderMode::Constant(L8::new(7)), [7, 7]),
        (BorderMode::Replicate, [10, 10]),
        (BorderMode::Reflect, [20, 10]),
        (BorderMode::Reflect101, [30, 20]),
        (BorderMode::Wrap, [30, 40]),
    ];
    for (border, edge) in cases {
        let out: Image<L8> = correlate2d(image.view(), &kernel, border).unwrap();
        assert_eq!(l8_values(&out), [edge[0], edge[1], 10, 20], "{border:?}");
    }
}

#[test]
fn vertical_kernels_use_the_rows_above_and_below() {
    // Diferença central vertical: abaixo − acima.
    let kernel = Kernel::from_rows([[-1.0], [0.0], [1.0]]).unwrap();
    let image = l8(2, &[1, 2, 10, 20, 100, 200]);
    let out: Image<LF32> = correlate2d(image.view(), &kernel, BorderMode::Replicate).unwrap();
    assert_eq!(lf32_values(&out), [9.0, 18.0, 99.0, 198.0, 90.0, 180.0]);
}

#[test]
fn integer_output_saturates_and_float_output_does_not() {
    let image = l8(2, &[200, 10]);
    let border = BorderMode::Replicate;

    let double = Kernel::from_rows([[2.0]]).unwrap();
    let out: Image<L8> = correlate2d(image.view(), &double, border).unwrap();
    assert_eq!(l8_values(&out), [255, 20]);
    let out: Image<LF32> = correlate2d(image.view(), &double, border).unwrap();
    assert_eq!(lf32_values(&out), [400.0, 20.0]);

    let negate = Kernel::from_rows([[-1.0]]).unwrap();
    let out: Image<L8> = correlate2d(image.view(), &negate, border).unwrap();
    assert_eq!(l8_values(&out), [0, 0]);
    let out: Image<LF32> = correlate2d(image.view(), &negate, border).unwrap();
    assert_eq!(lf32_values(&out), [-200.0, -10.0]);
}

#[test]
fn integer_output_rounds_half_away_from_zero() {
    // Média de dois vizinhos: (1 + 2)/2 = 1,5 → 2; (2 + 4)/2 = 3.
    let kernel = Kernel::from_rows([[0.0, 0.5, 0.5]]).unwrap();
    let out: Image<L8> =
        correlate2d(l8(3, &[1, 2, 4]).view(), &kernel, BorderMode::Replicate).unwrap();
    assert_eq!(l8_values(&out), [2, 3, 4]);
}

#[test]
fn every_channel_is_filtered_independently() {
    let image = Image::from_fn(3, 1, |x, _| {
        let v = u8::try_from(x).unwrap();
        Rgb8::new(v, 10 * v, 100 * v)
    })
    .unwrap();
    let kernel = Kernel::from_rows([[1.0, 1.0, 1.0]]).unwrap();
    let out: Image<Rgb8> = correlate2d(image.view(), &kernel, BorderMode::Replicate).unwrap();
    // Somas de (0,0,1), (0,1,2), (1,2,2) por canal.
    assert_eq!(
        out.as_pixels(),
        [
            Rgb8::new(1, 10, 100),
            Rgb8::new(3, 30, 255),
            Rgb8::new(5, 50, 255)
        ]
    );
}

#[test]
fn a_kernel_larger_than_the_image_is_accepted() {
    let image = l8(2, &[10, 20, 30, 40]);
    let kernel = Kernel::new(7, 7, vec![1.0; 49]).unwrap();
    // Cada janela 7×7 dá várias voltas na imagem 2×2; o esperado vem da definição.
    for border in [
        BorderMode::Wrap,
        BorderMode::Reflect101,
        BorderMode::Reflect,
    ] {
        let out: Image<LF32> = correlate2d(image.view(), &kernel, border).unwrap();
        let expected = naive_correlate(image.view(), &kernel, border);
        for (got, want) in lf32_values(&out).into_iter().zip(expected) {
            assert_eq!(f64::from(got), want, "{border:?}");
        }
    }
}

#[test]
fn a_region_of_interest_is_filtered_like_a_copy_of_it() {
    let image = noise::<L8>(12, 9, 3);
    let region = image.roi(Rect::new(2, 1, 7, 5)).unwrap();
    let copy = region.to_image().unwrap();
    let kernel = Kernel::from_rows([[1.0, 2.0, 1.0], [0.0, 0.0, 0.0], [-1.0, -2.0, -1.0]]).unwrap();
    for border in [BorderMode::Replicate, BorderMode::Reflect101] {
        let from_view: Image<LF32> = correlate2d(region, &kernel, border).unwrap();
        let from_copy: Image<LF32> = correlate2d(copy.view(), &kernel, border).unwrap();
        assert_images_eq!(from_view, from_copy);
    }
}

#[test]
fn sobel_magnitude_is_two_correlations_with_zero_border() {
    let image = noise::<L8>(17, 11, 7);
    let gx = Kernel::from_rows([[-1.0, 0.0, 1.0], [-2.0, 0.0, 2.0], [-1.0, 0.0, 1.0]]).unwrap();
    let gy = Kernel::from_rows([[-1.0, -2.0, -1.0], [0.0, 0.0, 0.0], [1.0, 2.0, 1.0]]).unwrap();
    let zero = BorderMode::Constant(L8::new(0));
    let dx: Image<LF32> = correlate2d(image.view(), &gx, zero).unwrap();
    let dy: Image<LF32> = correlate2d(image.view(), &gy, zero).unwrap();

    let sobel = apply_sobel(image.view()).unwrap();
    for ((dx, dy), want) in dx
        .as_pixels()
        .iter()
        .zip(dy.as_pixels())
        .zip(sobel.as_pixels())
    {
        let magnitude = dx.luma().hypot(dy.luma()).min(255.0);
        // `apply_sobel` trunca a magnitude.
        assert_eq!(magnitude.floor(), f32::from(want.luma()));
    }
}

/// Esperados do `cv2.filter2D(img, CV_32F, k, borderType=…)` do `OpenCV` 5.0.0
/// para a mesma imagem 5×4 e o mesmo kernel assimétrico.
#[test]
fn matches_opencv_filter2d() {
    let image = Image::from_fn(5, 4, |x, y| {
        L8::new(u8::try_from((x * 37 + y * 91 + x * y * 13) % 256).unwrap())
    })
    .unwrap();
    let kernel = Kernel::from_rows([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, -9.0]]).unwrap();
    let cases: [(BorderMode<L8>, [i16; 20]); 3] = [
        (
            BorderMode::Reflect101,
            [
                1562, 1621, 2776, 5467, 2682, 2979, 5277, 4579, 2601, 4519, 4056, 2553, 2974, 5187,
                3104, 3325, 5141, 4477, 2789, 4785,
            ],
        ),
        (
            BorderMode::Replicate,
            [
                429, 971, 2048, 5429, 4575, 2301, 5277, 4579, 2601, 2827, 3222, 2553, 2974, 5187,
                4484, 1410, 1717, 3179, 5665, 4350,
            ],
        ),
        (
            BorderMode::Constant(L8::new(0)),
            [
                -319, 675, 1530, 4689, 3151, 663, 5277, 4579, 2601, 3775, 2284, 2553, 2974, 5187,
                3896, 1742, 2375, 3381, 3107, 1776,
            ],
        ),
    ];
    for (border, expected) in cases {
        let out: Image<LF32> = correlate2d(image.view(), &kernel, border).unwrap();
        assert_eq!(lf32_values(&out), expected.map(f32::from), "{border:?}");
    }
}

#[test]
fn output_with_another_channel_count_is_an_error() {
    let image = l8(2, &[1, 2]);
    let err = correlate2d::<L8, RgbF32>(image.view(), &impulse_kernel(3, 3), BorderMode::Replicate)
        .unwrap_err();
    assert!(matches!(
        err,
        Error::ChannelMismatch {
            expected: 3,
            actual: 1
        }
    ));
}

// ── Propriedades ───────────────────────────────────────────────────────────────

fn assert_identity<P: Pixel>(image: &Image<P>, width: usize, height: usize)
where
    P::Subpixel: perceptor_imgproc::convert::Sample,
{
    let kernel = impulse_kernel(width, height);
    let correlated: Image<P> = correlate2d(image.view(), &kernel, BorderMode::Reflect101).unwrap();
    assert_images_eq!(correlated, *image);
    let convolved: Image<P> = convolve2d(image.view(), &kernel, BorderMode::Reflect101).unwrap();
    assert_images_eq!(convolved, *image);
}

proptest! {
    #[test]
    fn impulse_kernel_is_the_identity(
        gray in small_image::<L8>(),
        color in small_image::<Rgb8>(),
        float in small_image::<LF32>(),
        (width, height) in (odd_side(), odd_side()),
        border in any_border(),
    ) {
        assert_identity(&gray, width, height);
        assert_identity(&color, width, height);
        assert_identity(&float, width, height);
        // O impulso só lê o centro: a borda não pode influir.
        let out: Image<L8> = correlate2d(gray.view(), &impulse_kernel(width, height), border).unwrap();
        prop_assert_eq!(l8_values(&out), l8_values(&gray));
    }

    #[test]
    fn correlation_is_additive_in_f32(
        (a, b) in (1usize..=12, 1usize..=12).prop_flat_map(|(w, h)| {
            (any_image::<L8>(w..=w, h..=h), any_image::<L8>(w..=w, h..=h))
        }),
        kernel in integer_kernel(),
        border in prop_oneof![
            Just(BorderMode::Constant(LF32::new(0.0))),
            Just(BorderMode::Replicate),
            Just(BorderMode::Reflect),
            Just(BorderMode::Reflect101),
            Just(BorderMode::Wrap),
        ],
    ) {
        // Valores inteiros até 510 e pesos inteiros: |soma| ≤ 510·4·49 < 2²⁴, exato em f32.
        let (a, b) = (raw_f32(&a), raw_f32(&b));
        let sum = Image::from_fn(a.width(), a.height(), |x, y| {
            LF32::new(a.row(y)[x].luma() + b.row(y)[x].luma())
        }).unwrap();

        let of_sum: Image<LF32> = convolve2d(sum.view(), &kernel, border).unwrap();
        let conv_a: Image<LF32> = convolve2d(a.view(), &kernel, border).unwrap();
        let conv_b: Image<LF32> = convolve2d(b.view(), &kernel, border).unwrap();
        let sum_of: Vec<f32> = lf32_values(&conv_a)
            .into_iter()
            .zip(lf32_values(&conv_b))
            .map(|(x, y)| x + y)
            .collect();
        prop_assert_eq!(lf32_values(&of_sum), sum_of);
    }

    #[test]
    fn correlation_is_homogeneous_in_f32(
        image in small_image::<L8>(),
        kernel in integer_kernel(),
        factor in -3i8..=3,
    ) {
        let factor = f32::from(factor);
        let image = raw_f32(&image);
        let scaled = Image::from_fn(image.width(), image.height(), |x, y| {
            LF32::new(factor * image.row(y)[x].luma())
        }).unwrap();
        let of_scaled: Image<LF32> = convolve2d(scaled.view(), &kernel, BorderMode::Reflect101).unwrap();
        let base: Image<LF32> = convolve2d(image.view(), &kernel, BorderMode::Reflect101).unwrap();
        let expected: Vec<f32> = lf32_values(&base).into_iter().map(|v| factor * v).collect();
        prop_assert_eq!(lf32_values(&of_scaled), expected);
    }

    #[test]
    fn matches_the_definition_with_get_bordered(
        image in small_image::<L8>(),
        kernel in float_kernel(),
        border in any_border(),
    ) {
        let out: Image<LF32> = correlate2d(image.view(), &kernel, border).unwrap();
        let expected = naive_correlate(image.view(), &kernel, border);
        for (got, want) in lf32_values(&out).into_iter().zip(expected) {
            // Soma de até 49 termos de módulo ≤ 255: folga para o estreitamento a f32.
            prop_assert!((f64::from(got) - want).abs() <= 1e-2, "{got} != {want}");
        }
    }

    #[test]
    fn convolution_is_correlation_with_the_flipped_kernel(
        image in small_image::<L8>(),
        kernel in float_kernel(),
        border in any_border(),
    ) {
        let convolved: Image<LF32> = convolve2d(image.view(), &kernel, border).unwrap();
        let correlated: Image<LF32> = correlate2d(image.view(), &kernel.flipped(), border).unwrap();
        prop_assert_eq!(lf32_values(&convolved), lf32_values(&correlated));
    }

    #[test]
    fn a_normalized_box_keeps_a_constant_image(
        (width, height) in (1usize..=10, 1usize..=10),
        level in any::<u8>(),
        (kw, kh) in (odd_side(), odd_side()),
    ) {
        let image = Image::filled(width, height, L8::new(level)).unwrap();
        #[allow(clippy::cast_precision_loss)] // área ≤ 49
        let kernel = Kernel::new(kw, kh, vec![1.0 / (kw * kh) as f32; kw * kh]).unwrap();
        for border in [BorderMode::Replicate, BorderMode::Reflect, BorderMode::Reflect101, BorderMode::Wrap] {
            let out: Image<L8> = correlate2d(image.view(), &kernel, border).unwrap();
            prop_assert_eq!(l8_values(&out), l8_values(&image));
        }
    }
}
