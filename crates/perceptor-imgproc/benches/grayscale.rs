//! Throughput da conversão para escala de cinza.
//!
//! ```text
//! just bench -p perceptor-imgproc --bench grayscale
//! ```

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use perceptor_core::Rgb8;
use perceptor_imgproc::grayscale::{convert_to_gray, reference, LumaWeights};
use perceptor_testkit::patterns::noise;

const SIZES: [(usize, usize); 3] = [(64, 64), (640, 480), (1920, 1080)];

fn grayscale(c: &mut Criterion) {
    let mut group = c.benchmark_group("grayscale");
    for (width, height) in SIZES {
        let image = noise::<Rgb8>(width, height, 7);
        let size = format!("{width}x{height}");
        group.throughput(Throughput::Elements((width * height) as u64));
        group.bench_with_input(
            BenchmarkId::new("convert_to_gray", &size),
            &image,
            |b, image| b.iter(|| convert_to_gray(black_box(image.view()))),
        );
        group.bench_with_input(BenchmarkId::new("reference", &size), &image, |b, image| {
            b.iter(|| reference::convert_to_gray(black_box(image.view()), LumaWeights::Bt601));
        });
    }
    group.finish();
}

criterion_group!(benches, grayscale);
criterion_main!(benches);
