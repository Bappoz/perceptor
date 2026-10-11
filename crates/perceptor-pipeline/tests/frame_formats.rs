//! `Frame` carrega qualquer formato de pixel e dá acesso tipado a ele.

use std::time::Duration;

use perceptor_core::{Error, Image, PixelFormat, Rgb8, Rgba8, L16, L8, LF32};
use perceptor_pipeline::{Frame, FrameData, FrameMeta};

fn meta() -> FrameMeta {
    FrameMeta::new(0, "test")
}

#[test]
fn frame_reports_format_and_geometry_of_any_pixel_type() {
    let rgb = Frame::new(meta(), Image::<Rgb8>::new(4, 3).unwrap());
    assert_eq!(rgb.format(), PixelFormat::Rgb8);
    assert_eq!((rgb.width(), rgb.height(), rgb.channels()), (4, 3, 3));
    assert!(!rgb.is_grayscale());

    let deep = Frame::new(meta(), Image::<L16>::new(2, 5).unwrap());
    assert_eq!(deep.format(), PixelFormat::L16);
    assert_eq!((deep.width(), deep.height(), deep.channels()), (2, 5, 1));
    assert!(deep.is_grayscale());

    let float = Frame::new(meta(), Image::<LF32>::new(1, 1).unwrap());
    assert_eq!(float.format(), PixelFormat::LF32);
}

#[test]
fn typed_access_returns_the_image_when_the_format_matches() {
    let image = Image::filled(2, 2, Rgb8::new(1, 2, 3)).unwrap();
    let mut frame = Frame::new(meta(), image);

    assert_eq!(
        frame.as_image::<Rgb8>().unwrap().get(1, 1),
        Some(&Rgb8::new(1, 2, 3))
    );

    frame.as_image_mut::<Rgb8>().unwrap().as_pixels_mut()[0] = Rgb8::new(9, 9, 9);
    assert_eq!(
        frame.as_image::<Rgb8>().unwrap().get(0, 0),
        Some(&Rgb8::new(9, 9, 9))
    );
}

#[test]
fn typed_access_with_the_wrong_format_is_an_error_not_a_panic() {
    let mut frame = Frame::new(meta(), Image::<Rgb8>::new(2, 2).unwrap());

    assert!(matches!(
        frame.as_image::<L8>(),
        Err(Error::FormatMismatch {
            expected: PixelFormat::L8,
            actual: PixelFormat::Rgb8
        })
    ));
    // Mesmo número de canais não basta: Rgba8 ≠ Rgb8, e Bgr8 ≠ Rgb8.
    assert!(frame.as_image_mut::<Rgba8>().is_err());
    assert!(frame.as_image::<perceptor_core::Bgr8>().is_err());
}

#[test]
fn into_image_gives_the_buffer_back_or_the_untouched_data() {
    let frame = Frame::new(meta(), Image::filled(2, 1, L8::new(7)).unwrap());

    let data = frame.data.into_image::<Rgb8>().unwrap_err();
    assert_eq!(data.format(), PixelFormat::L8, "os dados voltam intactos");

    let image = data.into_image::<L8>().unwrap();
    assert_eq!(image.as_pixels(), [L8::new(7), L8::new(7)]);
}

#[test]
fn replacing_the_data_changes_the_format() {
    let mut frame = Frame::new(meta(), Image::<Rgb8>::new(2, 2).unwrap());
    frame.data = FrameData::from(Image::<L8>::new(2, 2).unwrap());
    assert_eq!(frame.format(), PixelFormat::L8);
    assert!(frame.as_image::<L8>().is_ok());
}

#[test]
fn meta_shares_the_source_and_carries_a_duration_timestamp() {
    let meta = FrameMeta::new(3, "camera0").with_timestamp(Duration::from_millis(40));
    let copy = meta.clone();

    assert_eq!(meta.index, 3);
    assert_eq!(&*meta.source, "camera0");
    assert_eq!(meta.timestamp, Duration::from_millis(40));
    assert!(
        std::sync::Arc::ptr_eq(&meta.source, &copy.source),
        "clonar não copia a string"
    );
}
