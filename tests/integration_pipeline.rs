//! Testes de integração: pipeline completo de filtros e I/O.
//!
//! `grayscale_system` e `sobel_system` compartilham o stage `Process`; o
//! `FiltersPlugin` os encadeia, então o `GrayscaleTag` já é visível ao Sobel no
//! mesmo tick.

use perceptor::{
    pipeline::{
        frame::{Frame, FrameMeta},
        pipeline::Pipeline,
        plugins::filters::{
            grayscale::GrayscaleTag,
            sobel::{SobelMap, SobelTag},
            FiltersPlugin,
        },
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

/// Pipeline com `IoPlugin`: lê PNG → grayscale → escreve arquivo de saída em 1 tick.
#[test]
fn io_pipeline_writes_grayscale_output() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input_pipeline.png");
    let output = tmp.path().join("output_pipeline.png");
    create_edge_png(&input);

    let mut pipeline = Pipeline::builder()
        .add_plugin(perceptor::pipeline::plugins::io::IoPlugin {
            input_path: input,
            output_path: output.clone(),
            ..Default::default()
        })
        .add_plugin(FiltersPlugin::all())
        .build();

    pipeline.tick().unwrap();

    assert!(output.exists(), "arquivo de saída deve ser criado");
    let saved = image::open(&output).unwrap().into_luma8();
    assert_eq!(
        saved.dimensions(),
        (8, 8),
        "dimensões preservadas após grayscale"
    );
}

/// Pipeline de filtros com frame injetado: grayscale e Sobel no mesmo tick.
#[test]
fn filter_chain_grayscale_then_sobel() {
    let mut pipeline = Pipeline::builder().add_plugin(FiltersPlugin::all()).build();

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
    let meta = FrameMeta {
        index: 0,
        timestamp_us: 0,
        source: "test".into(),
    };
    pipeline.world_mut().spawn(Frame::new(meta, data));

    // Um tick basta: o Sobel é ordenado depois do grayscale e enxerga o `GrayscaleTag`.
    pipeline.tick().unwrap();

    let world = pipeline.world_mut();
    let mut q = world.query::<(&Frame, &GrayscaleTag, &SobelTag, &SobelMap)>();
    // single() verifica implicitamente que existe exatamente 1 resultado
    let (frame, _, _, sobel) = q.single(world).unwrap();
    assert_eq!(frame.channels(), 1, "frame deve ser grayscale (1 canal)");
    assert_eq!(sobel.magnitude.shape(), &[8, 8, 1]);
    assert!(
        (1..7usize).any(|x| sobel.magnitude[[4, x, 0]] > 100),
        "borda horizontal em y=4 deve ter alta resposta Sobel"
    );
}

/// Com `IoPlugin` o frame vive um tick: o Sobel tem de sair antes do despawn.
#[test]
fn io_pipeline_computes_sobel_before_the_frame_is_despawned() {
    use perceptor::pipeline::Stage;
    use perceptor::prelude::{Query, ResMut, Resource};

    #[derive(Resource, Default)]
    struct SobelSeen(usize);

    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input_sobel.png");
    create_edge_png(&input);

    let mut builder = Pipeline::builder()
        .add_plugin(perceptor::pipeline::plugins::io::IoPlugin {
            input_path: input,
            output_path: tmp.path().join("output_sobel.png"),
            ..Default::default()
        })
        .add_plugin(FiltersPlugin::all());
    builder.world_mut().init_resource::<SobelSeen>();
    builder.add_system(
        Stage::Output,
        |maps: Query<&SobelMap>, mut seen: ResMut<SobelSeen>| seen.0 += maps.iter().count(),
    );
    let mut pipeline = builder.build();

    pipeline.tick().unwrap();
    pipeline.tick().unwrap();

    assert_eq!(pipeline.world().resource::<SobelSeen>().0, 2);
    let world = pipeline.world_mut();
    assert_eq!(world.query::<&Frame>().iter(world).count(), 0);
}
