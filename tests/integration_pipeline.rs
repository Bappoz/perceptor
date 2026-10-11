//! Testes de integração: pipeline completo de filtros e I/O.
//!
//! `grayscale_system` e `sobel_system` compartilham o stage `Process`; o
//! `FiltersPlugin` os encadeia, então o `GrayscaleTag` já é visível ao Sobel no
//! mesmo tick.

use perceptor::{
    core::{Image, PixelFormat, Rgb8},
    pipeline::{
        frame::Frame,
        pipeline::Pipeline,
        plugins::filters::{
            grayscale::GrayscaleTag,
            sobel::{SobelMap, SobelTag},
            FiltersPlugin,
        },
        testing::TestPipeline,
    },
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
        .build()
        .unwrap();

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
    let mut test = TestPipeline::with_plugin(FiltersPlugin::all());

    // Frame RGB 8×8 com borda horizontal: metade branca / metade preta
    let frame = test.spawn(
        Image::from_fn(8, 8, |_, y| {
            let v = if y < 4 { 255 } else { 0 };
            Rgb8::new(v, v, v)
        })
        .unwrap(),
    );

    // Um tick basta: o Sobel é ordenado depois do grayscale e enxerga o `GrayscaleTag`.
    test.tick().unwrap();

    test.assert_has_all::<(GrayscaleTag, SobelTag, SobelMap)>(frame);
    assert_eq!(test.frame(frame).format(), PixelFormat::L8);
    let sobel = test.component::<SobelMap>(frame).unwrap();
    assert_eq!(sobel.magnitude.dimensions(), (8, 8));
    assert!(
        sobel.magnitude.row(4)[1..7].iter().any(|p| p.luma() > 100),
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
    let mut pipeline = builder.build().unwrap();

    pipeline.tick().unwrap();
    pipeline.tick().unwrap();

    assert_eq!(pipeline.world().resource::<SobelSeen>().0, 2);
    let world = pipeline.world_mut();
    assert_eq!(world.query::<&Frame>().iter(world).count(), 0);
}
