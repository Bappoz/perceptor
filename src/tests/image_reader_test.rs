use crate::{
    core::{frame::Frame, pipeline::Pipeline},
    plugins::{filters::FiltersPlugin, io::image_reader::IoConfig, io::IoPlugin},
};
use bevy_ecs::prelude::*;

#[test]
fn test_image_reader_spawns_frame() {
    let mut pipeline = Pipeline::builder()
        .add_plugin(IoPlugin {
            source: "src/tests/fixtures/cachorro.png".into(),
        })
        .build();
    // Verifica se IoConfig chegou no world

    let _ = tracing_subscriber::fmt().try_init();
    pipeline.tick().unwrap();
    let index = pipeline.world().resource::<IoConfig>().next_index;
    println!("next_index após tick: {index}");

    let mut count = 0;
    pipeline
        .world_mut()
        .query::<&Frame>()
        .iter(pipeline.world())
        .for_each(|f| {
            assert_eq!(f.channels(), 3);
            assert!(f.width() > 0);
            count += 1;
        });
    assert_eq!(count, 1);
}
