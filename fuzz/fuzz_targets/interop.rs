//! Conversões de/para `ndarray::ArrayView3` e `image::ImageBuffer`.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = perceptor_testkit::fuzz::interop_ndarray(data);
    let _ = perceptor_testkit::fuzz::interop_image(data);
});
