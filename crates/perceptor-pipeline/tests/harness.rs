//! O harness `TestPipeline`: injeta frames, roda ticks e verifica componentes.

use bevy_ecs::prelude::*;
use perceptor_core::{Error, Image, PixelFormat, Rgb8, L8};
use perceptor_pipeline::plugins::filters::{
    grayscale::GrayscaleTag,
    sobel::{SobelMap, SobelTag},
    FiltersPlugin,
};
use perceptor_pipeline::testing::TestPipeline;
use perceptor_pipeline::{
    despawn_processed_frames_system, fallible, ErrorPolicy, Frame, FrameMeta, Pipeline,
    PipelineErrors, Stage,
};

#[derive(Component, Debug, PartialEq)]
struct Brightness(u8);

/// Anexa a luminância do primeiro pixel de cada frame `L8`.
fn brightness_system(frames: Query<(Entity, &Frame)>, mut commands: Commands) {
    for (entity, frame) in &frames {
        if let Ok(image) = frame.as_image::<L8>() {
            commands
                .entity(entity)
                .insert(Brightness(image.as_pixels()[0].luma()));
        }
    }
}

fn gray(luma: u8) -> Image<L8> {
    Image::filled(2, 2, L8::new(luma)).unwrap()
}

#[test]
fn spawned_images_become_frames_with_sequential_indices() {
    let mut test = TestPipeline::new();
    let a = test.spawn(gray(1));
    let b = test.spawn(Image::<Rgb8>::new(3, 2).unwrap());

    assert_eq!(test.frame(a).meta.index, 0);
    assert_eq!(test.frame(b).meta.index, 1);
    assert_eq!(test.frame(b).format(), PixelFormat::Rgb8);
    assert_eq!(test.image::<L8>(a).as_pixels()[0], L8::new(1));
    assert_eq!(test.frames(), [a, b]);
}

#[test]
fn a_system_under_test_runs_on_the_injected_frames() {
    let mut test = TestPipeline::with_systems(Stage::Process, brightness_system);
    let dark = test.spawn(gray(10));
    let bright = test.spawn(gray(200));
    let color = test.spawn(Image::<Rgb8>::new(2, 2).unwrap());

    test.tick().unwrap();

    assert_eq!(test.component::<Brightness>(dark), Some(&Brightness(10)));
    assert_eq!(test.component::<Brightness>(bright), Some(&Brightness(200)));
    test.assert_has::<Brightness>(dark);
    test.assert_lacks::<Brightness>(color);
}

#[test]
fn a_plugin_under_test_is_checked_with_a_tuple_of_components() {
    let mut test = TestPipeline::with_plugin(FiltersPlugin::all());
    let frame = test.spawn(Image::<Rgb8>::new(4, 4).unwrap());
    let untouched = test.spawn(Image::<perceptor_core::LF32>::new(4, 4).unwrap());

    test.tick().unwrap();

    test.assert_has_all::<(GrayscaleTag, SobelTag, SobelMap)>(frame);
    test.assert_lacks_all::<(GrayscaleTag, SobelTag, SobelMap)>(untouched);
}

#[test]
#[should_panic(expected = "Brightness")]
fn assert_has_names_the_missing_component() {
    let mut test = TestPipeline::new();
    let frame = test.spawn(gray(0));
    test.assert_has_all::<(Frame, Brightness)>(frame);
}

#[test]
#[should_panic(expected = "Frame")]
fn assert_lacks_names_the_unexpected_component() {
    let mut test = TestPipeline::new();
    let frame = test.spawn(gray(0));
    test.assert_lacks_all::<(Brightness, Frame)>(frame);
}

#[test]
#[should_panic(expected = "FormatMismatch")]
fn typed_image_access_reports_the_actual_format() {
    let mut test = TestPipeline::new();
    let frame = test.spawn(gray(0));
    let _ = test.image::<Rgb8>(frame);
}

#[test]
fn despawned_frames_are_observable() {
    let mut test = TestPipeline::with_systems(Stage::Cleanup, despawn_processed_frames_system);
    let frame = test.spawn(gray(0));
    test.assert_alive(frame);

    test.run_ticks(2).unwrap();

    test.assert_despawned(frame);
    assert_eq!(test.frames(), []);
}

#[test]
#[should_panic(expected = "ainda existe")]
fn assert_despawned_fails_for_a_live_entity() {
    let mut test = TestPipeline::new();
    let frame = test.spawn(gray(0));
    test.assert_despawned(frame);
}

#[test]
fn errors_follow_the_policy_of_the_builder() {
    fn fails() -> perceptor_core::Result<()> {
        Err(Error::NonContiguous)
    }

    let mut test = TestPipeline::with_systems(Stage::Input, fallible(fails));
    assert!(matches!(test.tick(), Err(Error::NonContiguous)));

    let mut builder = Pipeline::builder().with_error_policy(ErrorPolicy::SkipFrame);
    builder.add_system(
        Stage::Process,
        |frames: Query<Entity, With<Frame>>, mut errors: ResMut<PipelineErrors>| {
            for frame in &frames {
                errors.report_frame("reject", frame, Error::NonContiguous);
            }
        },
    );
    let mut test = TestPipeline::from_builder(builder);
    let frame = test.spawn(gray(0));

    test.tick().unwrap();

    assert_eq!(test.errors().len(), 1);
    test.assert_despawned(frame);
}

#[test]
fn custom_metadata_is_kept_by_spawn_frame() {
    let mut test = TestPipeline::new();
    let frame = test.spawn_frame(Frame::new(FrameMeta::new(42, "camera"), gray(0)));
    assert_eq!(test.frame(frame).meta.index, 42);
    // O contador automático segue independente dos índices escolhidos à mão.
    let next = test.spawn(gray(0));
    assert_eq!(test.frame(next).meta.index, 0);
}
