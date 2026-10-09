//! Erros tipados das funções de leitura e escrita de frames.

use perceptor_core::Error;
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
    assert_eq!(frame.meta.source, FIXTURE);
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
fn write_frame_rejects_unsupported_channel_count() {
    let meta = FrameMeta {
        index: 0,
        timestamp_us: 0,
        source: "test".into(),
    };
    let frame = Frame::new(meta, ndarray::Array3::zeros((2, 2, 2)));
    let dir = tempfile::tempdir().unwrap();

    assert!(matches!(
        write_frame(&frame, &dir.path().join("out.png"), ImageFormat::Png),
        Err(Error::UnsupportedChannels { channels: 2 })
    ));
}

#[test]
fn write_then_read_round_trips_png() {
    let frame = read_frame(FIXTURE.as_ref(), 0).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.png");

    write_frame(&frame, &out, ImageFormat::Png).unwrap();

    assert_eq!(read_frame(&out, 0).unwrap().data, frame.data);
}
