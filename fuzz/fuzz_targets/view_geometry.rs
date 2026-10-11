//! `ImageView`/`ImageViewMut`: geometria, ROI, divisão em faixas e bordas.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = perceptor_testkit::fuzz::view_geometry(data);
});
