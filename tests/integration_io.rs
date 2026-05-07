//! Testes de integração: leitura e escrita de imagens via IoPlugin.

use perceptor::{
    core::{
        frame::Frame,
        pipeline::{Pipeline, PipelineState},
    },
    plugins::io::IoPlugin,
};

fn create_test_png(path: &std::path::Path, width: u32, height: u32) {
    let mut img = image::RgbImage::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = image::Rgb([(x * 40) as u8, (y * 30) as u8, 100u8]);
    }
    img.save(path).unwrap();
}

/// Lê uma imagem PNG e verifica que exatamente 1 entidade Frame é criada com shape correto.
#[test]
fn reads_image_and_spawns_frame_entity() {
    let tmp = std::env::temp_dir().join("perceptor_io_test_read");
    std::fs::create_dir_all(&tmp).unwrap();
    let input = tmp.join("input.png");
    let output = tmp.join("output_read.png");
    create_test_png(&input, 4, 6);

    let mut pipeline = Pipeline::builder()
        .add_plugin(IoPlugin {
            input_path: input,
            output_path: output,
            ..Default::default()
        })
        .build();

    pipeline.tick().unwrap();

    let world = pipeline.world_mut();
    let mut q = world.query::<&Frame>();
    let frames: Vec<_> = q.iter(world).collect();
    assert_eq!(frames.len(), 1, "deve existir exatamente 1 entidade Frame");
    assert_eq!(frames[0].channels(), 3, "PNG RGB deve ter 3 canais");
    assert_eq!(frames[0].height(), 6);
    assert_eq!(frames[0].width(), 4);
}

/// Verifica que path inválido define should_stop=true e não spawna frames.
#[test]
fn sets_should_stop_on_invalid_path() {
    let tmp = std::env::temp_dir().join("perceptor_io_test_err");
    std::fs::create_dir_all(&tmp).unwrap();

    let mut pipeline = Pipeline::builder()
        .add_plugin(IoPlugin {
            input_path: tmp.join("nonexistent_xyz.png"),
            output_path: tmp.join("output_err.png"),
            ..Default::default()
        })
        .build();

    pipeline.tick().unwrap();

    let world = pipeline.world_mut();
    assert!(
        world.resource::<PipelineState>().should_stop,
        "should_stop deve ser true para path inválido"
    );
    let mut q = world.query::<&Frame>();
    assert_eq!(q.iter(world).count(), 0, "nenhum frame deve ser spawned");
}
