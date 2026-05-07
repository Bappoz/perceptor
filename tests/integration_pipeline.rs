//! Testes de integração: pipeline completo de filtros e I/O.
//!
//! Nota: grayscale_system e sobel_system compartilham o ProcessStage, mas os
//! comandos ECS são flushed ao final do schedule. O GrayscaleTag inserido pelo
//! grayscale_system só é visível ao sobel_system no tick seguinte.

use perceptor::{
    core::{
        frame::{Frame, FrameMeta},
        pipeline::Pipeline,
    },
    plugins::filters::{
        grayscale::GrayscaleTag,
        sobel::{SobelMap, SobelTag},
        FiltersPlugin,
    },
    prelude::Array3,
};

/// PNG 8×8 com borda horizontal nítida: metade superior branca, inferior preta.
fn create_edge_png(path: &std::path::Path) {
    let mut img = image::RgbImage::new(8, 8);
    for (_, y, pixel) in img.enumerate_pixels_mut() {
        let v = if y < 4 { 255u8 } else { 0u8 };
        *pixel = image::Rgb([v, v, v]);
    }
    img.save(path).unwrap();
}

/// Pipeline com IoPlugin: lê PNG → grayscale → escreve arquivo de saída em 1 tick.
#[test]
fn io_pipeline_writes_grayscale_output() {
    let tmp = std::env::temp_dir().join("perceptor_pipeline_io");
    std::fs::create_dir_all(&tmp).unwrap();
    let input = tmp.join("input_pipeline.png");
    let output = tmp.join("output_pipeline.png");
    create_edge_png(&input);

    let mut pipeline = Pipeline::builder()
        .add_plugin(perceptor::plugins::io::IoPlugin {
            input_path: input,
            output_path: output.clone(),
            ..Default::default()
        })
        .add_plugin(FiltersPlugin::all())
        .build();

    pipeline.tick().unwrap();

    assert!(output.exists(), "arquivo de saída deve ser criado");
    let saved = image::open(&output).unwrap().into_luma8();
    assert_eq!(saved.dimensions(), (8, 8), "dimensões preservadas após grayscale");
}

/// Pipeline de filtros com frame injetado: grayscale no tick 1, sobel no tick 2.
#[test]
fn filter_chain_grayscale_then_sobel() {
    let mut pipeline = Pipeline::builder()
        .add_plugin(FiltersPlugin::all())
        .build();

    // Frame RGB 8×8 com borda horizontal: metade branca / metade preta
    let mut flat = vec![0u8; 8 * 8 * 3];
    for y in 0..8usize {
        let v = if y < 4 { 255u8 } else { 0u8 };
        for x in 0..8usize {
            let i = (y * 8 + x) * 3;
            flat[i] = v;
            flat[i + 1] = v;
            flat[i + 2] = v;
        }
    }
    let data = Array3::from_shape_vec((8, 8, 3), flat).unwrap();
    let meta = FrameMeta { index: 0, timestamp_us: 0, source: "test".into() };
    pipeline.world_mut().spawn(Frame::new(meta, data));

    // Tick 1: grayscale_system converte RGB → cinza e insere GrayscaleTag
    pipeline.tick().unwrap();
    {
        let world = pipeline.world_mut();
        let mut q = world.query::<(&Frame, &GrayscaleTag)>();
        // single() verifica implicitamente que existe exatamente 1 resultado
        let (frame, _) = q.single(world);
        assert_eq!(frame.channels(), 1, "frame deve ser grayscale (1 canal)");
    }

    // Tick 2: sobel_system encontra GrayscaleTag e insere SobelTag + SobelMap
    pipeline.tick().unwrap();
    {
        let world = pipeline.world_mut();
        let mut q = world.query::<(&Frame, &GrayscaleTag, &SobelTag, &SobelMap)>();
        let (frame, _, _, sobel) = q.single(world);
        assert_eq!(frame.channels(), 1);
        assert_eq!(sobel.magnitude.shape(), &[8, 8, 1]);
        assert!(
            (1..7usize).any(|x| sobel.magnitude[[4, x, 0]] > 100),
            "borda horizontal em y=4 deve ter alta resposta Sobel"
        );
    }
}
