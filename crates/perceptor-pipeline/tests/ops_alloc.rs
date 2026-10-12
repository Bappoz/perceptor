//! Uma op in-place não aloca.
//!
//! Em binário de teste próprio: mede com um alocador global que conta, por
//! thread, as alocações feitas enquanto a medição está ligada.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use perceptor_core::{Image, ImageView, ImageViewMut, Result, L8};
use perceptor_pipeline::frame::{Frame, FrameMeta};
use perceptor_pipeline::ops::Op;
use perceptor_pipeline::testing::TestPipeline;
use perceptor_pipeline::{Pipeline, Stage};

thread_local! {
    static MEASURING: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
    static LARGEST: Cell<usize> = const { Cell::new(0) };
}

struct Counting;

// SAFETY: toda chamada é repassada sem alteração ao `System`, que cumpre o
// contrato de `GlobalAlloc`. A contagem só toca `Cell`s thread-local
// inicializadas com `const`, que não alocam nem registram destrutor;
// `try_with` cobre o acesso durante o encerramento da thread.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = MEASURING.try_with(|on| {
            if on.get() {
                let _ = COUNT.try_with(|c| c.set(c.get() + 1));
                let _ = LARGEST.try_with(|l| l.set(l.get().max(layout.size())));
            }
        });
        // SAFETY: mesmo `layout` recebido, cujo contrato quem chama garante.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` veio de `System.alloc` com este `layout`, por `alloc` acima.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Alocações feitas por `f` nesta thread: `(quantidade, maior em bytes)`.
fn allocations(f: impl FnOnce()) -> (usize, usize) {
    COUNT.set(0);
    LARGEST.set(0);
    MEASURING.set(true);
    f();
    MEASURING.set(false);
    (COUNT.get(), LARGEST.get())
}

const SIDE: usize = 256;
const IMAGE_BYTES: usize = SIDE * SIDE;

fn invert(mut image: ImageViewMut<'_, L8>) {
    for pixel in image.pixels_mut() {
        *pixel = L8::new(255 - pixel.luma());
    }
}

fn copy(image: ImageView<'_, L8>) -> Result<Image<L8>> {
    Image::from_fn(image.width(), image.height(), |x, y| image.row(y)[x])
}

fn frame() -> Frame {
    let image = Image::filled(SIDE, SIDE, L8::new(1)).unwrap();
    Frame::new(FrameMeta::new(0, "alloc"), image)
}

#[test]
fn the_counter_sees_an_image_allocation() {
    let (count, largest) = allocations(|| drop(frame()));
    assert!(count >= 1);
    assert!(largest >= IMAGE_BYTES);
}

#[test]
fn applying_an_in_place_op_does_not_allocate() {
    let mut frame = frame();

    let (count, _) = allocations(|| Op::apply(&invert, &mut frame).unwrap());

    assert_eq!(count, 0);
    assert_eq!(frame.as_image::<L8>().unwrap().row(0)[0], L8::new(254));
}

#[test]
fn applying_a_map_op_allocates_the_output_image() {
    let mut frame = frame();

    let (count, largest) = allocations(|| Op::apply(&copy, &mut frame).unwrap());

    assert!(count >= 1);
    assert!(largest >= IMAGE_BYTES);
}

/// No pipeline inteiro o tick tem alocações próprias (schedule, spans), mas
/// nenhuma do tamanho de uma imagem quando a op é in-place.
#[test]
fn a_tick_with_an_in_place_op_allocates_no_image_buffer() {
    let largest_in_tick = |in_place: bool| {
        let mut builder = Pipeline::builder();
        if in_place {
            builder.add_op(Stage::Process, invert);
        } else {
            builder.add_op(Stage::Process, copy);
        }
        let mut test = TestPipeline::from_builder(builder);
        test.spawn_frame(frame());
        // O primeiro tick inicializa o schedule.
        test.tick().unwrap();
        allocations(|| test.tick().unwrap()).1
    };

    assert!(
        largest_in_tick(false) >= IMAGE_BYTES,
        "controle: a op que copia aloca"
    );
    assert!(largest_in_tick(true) < IMAGE_BYTES);
}
