//! Leitura de uma imagem real do disco pelo `IoPlugin`.

use perceptor_pipeline::{
    frame::Frame,
    pipeline::Pipeline,
    plugins::io::{config::ImageFormat, IoPlugin},
};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cachorro.png");

#[test]
fn image_reader_spawns_frame_from_fixture() {
    let out = tempfile::tempdir().unwrap();
    let mut pipeline = Pipeline::builder()
        .add_plugin(IoPlugin {
            input_path: FIXTURE.into(),
            output_path: out.path().join("output.png"),
            format: ImageFormat::Png,
        })
        .build();

    pipeline.tick().unwrap();

    let world = pipeline.world_mut();
    let mut q = world.query::<&Frame>();
    let frames: Vec<_> = q.iter(world).collect();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].channels(), 3);
    assert_eq!((frames[0].width(), frames[0].height()), (500, 333));
}
