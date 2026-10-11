//! Erros tipados das funções de leitura e escrita de frames.

use perceptor_core::{Bgr8, Error, Image, La8, PixelFormat, Rgb8, L16, LF32};
use perceptor_pipeline::frame::{Frame, FrameMeta};
use perceptor_pipeline::plugins::io::{
    config::ImageFormat, image_reader::read_frame, image_writer::write_frame,
};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cachorro.png");

#[test]
fn read_frame_reports_missing_file_as_decode_error() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nao_existe.png");

    let err = read_frame(&missing, 0).unwrap_err();

    assert!(matches!(&err, Error::Decode { path, .. } if path == &missing));
}

#[test]
fn read_frame_reports_corrupt_file_as_decode_error() {
    let dir = tempfile::tempdir().unwrap();
    let corrupt = dir.path().join("lixo.png");
    std::fs::write(&corrupt, b"isto nao e um png").unwrap();

    assert!(matches!(read_frame(&corrupt, 0), Err(Error::Decode { .. })));
}

#[test]
fn read_frame_sets_index_and_source() {
    let frame = read_frame(FIXTURE.as_ref(), 7).unwrap();

    assert_eq!(frame.meta.index, 7);
    assert_eq!(&*frame.meta.source, FIXTURE);
    assert_eq!(
        (frame.width(), frame.height(), frame.channels()),
        (500, 333, 3)
    );
}

#[test]
fn write_frame_reports_unwritable_path_as_encode_error() {
    let frame = read_frame(FIXTURE.as_ref(), 0).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("pasta_inexistente").join("out.png");

    let err = write_frame(&frame, &bad, ImageFormat::Png).unwrap_err();

    assert!(matches!(&err, Error::Encode { path, .. } if path == &bad));
}

#[test]
fn write_frame_rejects_formats_without_a_file_representation() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.png");

    let bgr = Frame::new(FrameMeta::new(0, "test"), Image::<Bgr8>::new(2, 2).unwrap());
    assert!(matches!(
        write_frame(&bgr, &out, ImageFormat::Png),
        Err(Error::UnsupportedFormat {
            format: PixelFormat::Bgr8
        })
    ));

    let float = Frame::new(FrameMeta::new(0, "test"), Image::<LF32>::new(2, 2).unwrap());
    assert!(matches!(
        write_frame(&float, &out, ImageFormat::Png),
        Err(Error::UnsupportedFormat {
            format: PixelFormat::LF32
        })
    ));
    assert!(!out.exists());
}

#[test]
fn write_frame_keeps_16_bit_depth_and_alpha_in_png() {
    let dir = tempfile::tempdir().unwrap();

    let deep = Image::from_fn(3, 2, |x, y| {
        L16::new(u16::try_from(x * 1000 + y * 300).unwrap())
    });
    let out = dir.path().join("deep.png");
    write_frame(
        &Frame::new(FrameMeta::new(0, "test"), deep.unwrap()),
        &out,
        ImageFormat::Png,
    )
    .unwrap();
    let saved = image::open(&out).unwrap();
    assert_eq!(saved.color(), image::ColorType::L16);
    assert_eq!(saved.as_luma16().unwrap().get_pixel(2, 1).0, [2300]);

    let alpha = Image::filled(2, 2, La8::new(10, 20)).unwrap();
    let out = dir.path().join("alpha.png");
    write_frame(
        &Frame::new(FrameMeta::new(0, "test"), alpha),
        &out,
        ImageFormat::Png,
    )
    .unwrap();
    assert_eq!(image::open(&out).unwrap().color(), image::ColorType::La8);
}

#[test]
fn write_frame_reports_pixel_format_the_file_format_cannot_hold() {
    let dir = tempfile::tempdir().unwrap();
    let frame = Frame::new(FrameMeta::new(0, "test"), Image::<L16>::new(2, 2).unwrap());

    assert!(matches!(
        write_frame(&frame, &dir.path().join("out.jpg"), ImageFormat::Jpeg),
        Err(Error::Encode { .. })
    ));
}

#[test]
fn write_then_read_round_trips_png() {
    let frame = read_frame(FIXTURE.as_ref(), 0).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.png");

    write_frame(&frame, &out, ImageFormat::Png).unwrap();

    let reread = read_frame(&out, 0).unwrap();
    assert_eq!(
        reread.as_image::<Rgb8>().unwrap().as_pixels(),
        frame.as_image::<Rgb8>().unwrap().as_pixels()
    );
}
