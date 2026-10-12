//! `PipelineBuilder::add_op`: kernels puros registrados como sistemas.

use bevy_ecs::prelude::*;
use perceptor_core::{Error, Image, ImageView, ImageViewMut, PixelFormat, Result, Rgb8, L8};
use perceptor_imgproc::{grayscale::convert_to_gray, sobel::apply_sobel};
use perceptor_pipeline::ops::Op;
use perceptor_pipeline::testing::TestPipeline;
use perceptor_pipeline::{ErrorPolicy, Pipeline, Stage};

fn invert(mut image: ImageViewMut<'_, L8>) {
    for pixel in image.pixels_mut() {
        *pixel = L8::new(255 - pixel.luma());
    }
}

fn add_ten(mut image: ImageViewMut<'_, L8>) {
    for pixel in image.pixels_mut() {
        *pixel = L8::new(pixel.luma().saturating_add(10));
    }
}

fn double(mut image: ImageViewMut<'_, L8>) {
    for pixel in image.pixels_mut() {
        *pixel = L8::new(pixel.luma().saturating_mul(2));
    }
}

// A assinatura de uma op in-place recebe a visão por valor.
#[allow(clippy::needless_pass_by_value)]
fn reject_small(image: ImageViewMut<'_, L8>) -> Result<()> {
    if image.width() < 4 {
        return Err(Error::InvalidDimensions {
            width: image.width(),
            height: image.height(),
        });
    }
    Ok(())
}

fn failing_map(_: ImageView<'_, L8>) -> Result<Image<L8>> {
    Err(Error::NonContiguous)
}

#[derive(Component, Default)]
struct Inverted;

#[derive(Component, Default)]
struct Gray;

fn gradient() -> Image<Rgb8> {
    Image::from_fn(8, 8, |x, y| {
        let v = u8::try_from(x * 30 + y).unwrap();
        Rgb8::new(v, v / 2, 255 - v)
    })
    .unwrap()
}

fn with_ops(build: impl FnOnce(&mut perceptor_pipeline::PipelineBuilder)) -> TestPipeline {
    let mut builder = Pipeline::builder();
    build(&mut builder);
    TestPipeline::from_builder(builder)
}

#[test]
fn map_op_replaces_the_frame_with_the_kernel_output() {
    let mut test = with_ops(|b| {
        b.add_op(Stage::Process, convert_to_gray);
    });
    let input = gradient();
    let expected = convert_to_gray(input.view()).unwrap();
    let frame = test.spawn(input);

    test.tick().unwrap();

    assert_eq!(test.frame(frame).format(), PixelFormat::L8);
    assert_eq!(test.image::<L8>(frame).as_pixels(), expected.as_pixels());
}

#[test]
fn in_place_op_mutates_the_frame() {
    let mut test = with_ops(|b| {
        b.add_op(Stage::Process, invert);
    });
    let frame = test.spawn(Image::filled(2, 2, L8::new(10)).unwrap());

    test.tick().unwrap();

    assert_eq!(test.image::<L8>(frame).as_pixels(), [L8::new(245); 4]);
}

#[test]
fn frames_in_another_format_are_skipped_without_error() {
    let mut test = with_ops(|b| {
        b.add_op(Stage::Process, invert);
        b.add_op(Stage::Process, apply_sobel);
    });
    let frame = test.spawn(Image::filled(2, 2, Rgb8::new(1, 2, 3)).unwrap());

    test.tick().unwrap();

    assert_eq!(
        test.image::<Rgb8>(frame).as_pixels(),
        [Rgb8::new(1, 2, 3); 4]
    );
    assert!(test.errors().is_empty());
}

#[test]
fn ops_in_the_same_stage_run_in_registration_order() {
    // Não comutam: ((v + 10) · 2 + 10) · 2 ≠ qualquer outra ordem.
    let mut test = with_ops(|b| {
        b.add_op(Stage::Process, add_ten)
            .add_op(Stage::Process, double)
            .add_op(Stage::Process, add_ten)
            .add_op(Stage::Process, double);
    });
    let frame = test.spawn(Image::filled(2, 2, L8::new(1)).unwrap());

    test.tick().unwrap();

    assert_eq!(test.image::<L8>(frame).as_pixels(), [L8::new(64); 4]);
}

#[test]
fn chained_ops_see_the_format_produced_by_the_previous_one() {
    let mut test = with_ops(|b| {
        b.add_op(Stage::Process, convert_to_gray)
            .add_op(Stage::Process, apply_sobel)
            .add_op(Stage::Process, invert);
    });
    let input = gradient();
    let mut expected = apply_sobel(convert_to_gray(input.view()).unwrap().view()).unwrap();
    invert(expected.view_mut());
    let frame = test.spawn(input);

    test.tick().unwrap();

    assert_eq!(test.image::<L8>(frame).as_pixels(), expected.as_pixels());
}

#[test]
fn ops_in_different_stages_follow_the_stage_order() {
    let mut test = with_ops(|b| {
        b.add_op(Stage::PostProcess, double);
        b.add_op(Stage::PreProcess, add_ten);
    });
    let frame = test.spawn(Image::filled(2, 2, L8::new(1)).unwrap());

    test.tick().unwrap();

    assert_eq!(test.image::<L8>(frame).as_pixels(), [L8::new(22); 4]);
}

#[test]
fn untagged_op_runs_again_on_a_frame_that_stays() {
    let mut test = with_ops(|b| {
        b.add_op(Stage::Process, invert);
    });
    let frame = test.spawn(Image::filled(2, 2, L8::new(10)).unwrap());

    test.run_ticks(2).unwrap();

    assert_eq!(test.image::<L8>(frame).as_pixels(), [L8::new(10); 4]);
}

#[test]
fn tagged_op_runs_once_per_frame_and_inserts_the_tag() {
    let mut test = with_ops(|b| {
        b.add_op(Stage::Process, invert.tagged::<Inverted>());
    });
    let frame = test.spawn(Image::filled(2, 2, L8::new(10)).unwrap());

    test.run_ticks(3).unwrap();

    test.assert_has::<Inverted>(frame);
    assert_eq!(test.image::<L8>(frame).as_pixels(), [L8::new(245); 4]);
}

#[test]
fn tag_is_visible_to_the_next_op_of_the_same_stage() {
    #[derive(Resource, Default)]
    struct Seen(usize);

    let mut builder = Pipeline::builder();
    builder.world_mut().init_resource::<Seen>();
    builder.add_op(Stage::Process, convert_to_gray.tagged::<Gray>());
    builder.add_op(Stage::Process, invert);
    builder.add_system(
        Stage::PostProcess,
        |tagged: Query<(), With<Gray>>, mut seen: ResMut<Seen>| seen.0 += tagged.iter().count(),
    );
    let mut test = TestPipeline::from_builder(builder);
    let frame = test.spawn(gradient());

    test.tick().unwrap();

    test.assert_has::<Gray>(frame);
    assert_eq!(test.world_mut().resource::<Seen>().0, 1);
}

#[test]
fn op_error_is_reported_for_the_frame_and_leaves_it_untagged() {
    let mut builder = Pipeline::builder().with_error_policy(ErrorPolicy::Continue);
    builder.add_op(Stage::Process, reject_small.tagged::<Inverted>());
    let mut test = TestPipeline::from_builder(builder);
    let small = test.spawn(Image::<L8>::new(2, 2).unwrap());
    let large = test.spawn(Image::<L8>::new(4, 4).unwrap());

    test.tick().unwrap();

    let errors: Vec<_> = test.errors().iter().collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].frame, Some(small));
    assert!(
        errors[0].system.ends_with("reject_small"),
        "{}",
        errors[0].system
    );
    assert!(matches!(errors[0].error, Error::InvalidDimensions { .. }));
    test.assert_lacks::<Inverted>(small);
    test.assert_has::<Inverted>(large);
}

#[test]
fn failing_map_op_keeps_the_frame_and_follows_the_error_policy() {
    let mut builder = Pipeline::builder();
    builder.add_op(Stage::Process, failing_map);
    let mut test = TestPipeline::from_builder(builder);
    let frame = test.spawn(Image::filled(2, 2, L8::new(7)).unwrap());

    assert!(matches!(test.tick(), Err(Error::NonContiguous)));

    assert_eq!(test.image::<L8>(frame).as_pixels(), [L8::new(7); 4]);
}

#[test]
fn closures_are_ops_too() {
    let gain = 3;
    let mut test = with_ops(|b| {
        b.add_op(Stage::Process, move |mut image: ImageViewMut<'_, L8>| {
            for pixel in image.pixels_mut() {
                *pixel = L8::new(pixel.luma().saturating_mul(gain));
            }
        });
    });
    let frame = test.spawn(Image::filled(2, 2, L8::new(5)).unwrap());

    test.tick().unwrap();

    assert_eq!(test.image::<L8>(frame).as_pixels(), [L8::new(15); 4]);
}

#[test]
fn an_op_can_be_applied_to_a_frame_without_a_pipeline() {
    use perceptor_pipeline::frame::{Frame, FrameMeta};

    let mut frame = Frame::new(FrameMeta::new(0, "t"), gradient());
    assert_eq!(Op::input_format(&convert_to_gray), PixelFormat::Rgb8);

    Op::apply(&convert_to_gray, &mut frame).unwrap();

    assert_eq!(frame.format(), PixelFormat::L8);
    assert!(matches!(
        Op::apply(&convert_to_gray, &mut frame),
        Err(Error::FormatMismatch { .. })
    ));
}
