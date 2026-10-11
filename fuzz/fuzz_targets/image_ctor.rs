//! Construtores de `Image<P>` com dimensões e buffers arbitrários.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = perceptor_testkit::fuzz::image_ctor(data);
});
