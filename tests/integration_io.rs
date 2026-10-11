//! Testes de integração: leitura e escrita de imagens via `IoPlugin`.

use perceptor::core::Error;
use perceptor::pipeline::{
    frame::Frame,
    pipeline::{Pipeline, PipelineState},
    plugins::io::IoPlugin,
    ErrorPolicy,
};

fn create_test_png(path: &std::path::Path, width: u32, height: u32) {
    let mut img = image::RgbImage::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = image::Rgb([
            u8::try_from(x * 40).unwrap(),
            u8::try_from(y * 30).unwrap(),
            100u8,
        ]);
    }
    img.save(path).unwrap();
}

/// Lê uma imagem PNG e verifica que exatamente 1 entidade Frame é criada com shape correto.
#[test]
fn reads_image_and_spawns_frame_entity() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input.png");
    let output = tmp.path().join("output_read.png");
    create_test_png(&input, 4, 6);

    let mut pipeline = Pipeline::builder()
        .add_plugin(IoPlugin {
            input_path: input,
            output_path: output,
            // Sem isto o frame é despawnado em `Stage::Cleanup` do mesmo tick.
            keep_frames: true,
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

/// Caminho de entrada inválido: em `FailFast` (o padrão) `run()` devolve o erro.
#[test]
fn invalid_input_path_makes_run_fail_in_fail_fast() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("nonexistent_xyz.png");
    let output = tmp.path().join("output_err.png");

    let mut pipeline = Pipeline::builder()
        .add_plugin(IoPlugin {
            input_path: missing.clone(),
            output_path: output.clone(),
            ..Default::default()
        })
        .build();

    let err = pipeline.run().unwrap_err();

    assert!(matches!(&err, Error::Decode { path, .. } if path == &missing));
    assert!(!output.exists(), "o stage de saída não roda depois do erro");
    let world = pipeline.world_mut();
    let mut q = world.query::<&Frame>();
    assert_eq!(q.iter(world).count(), 0, "nenhum frame deve ser spawned");
}

/// Nas outras políticas o mesmo erro fica registrado e `run()` termina com `Ok`.
#[test]
fn invalid_input_path_is_recorded_without_failing_in_continue() {
    let tmp = tempfile::tempdir().unwrap();

    let mut pipeline = Pipeline::builder()
        .with_error_policy(ErrorPolicy::Continue)
        .add_plugin(IoPlugin {
            input_path: tmp.path().join("nonexistent_xyz.png"),
            output_path: tmp.path().join("output_err.png"),
            ..Default::default()
        })
        .build();

    pipeline.run().unwrap();

    assert!(pipeline.world().resource::<PipelineState>().should_stop);
    let errors: Vec<_> = pipeline.errors().iter().collect();
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0].error, Error::Decode { .. }));
    assert_eq!(errors[0].frame, None);
}

/// Falha de escrita é um erro do frame: `SkipFrame` o descarta e o pipeline segue.
#[test]
fn unwritable_output_discards_the_frame_in_skip_frame() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input.png");
    create_test_png(&input, 4, 6);
    let bad_output = tmp.path().join("pasta_inexistente").join("out.png");

    let mut pipeline = Pipeline::builder()
        .with_error_policy(ErrorPolicy::SkipFrame)
        .add_plugin(IoPlugin {
            input_path: input,
            output_path: bad_output.clone(),
            keep_frames: true,
            ..Default::default()
        })
        .build();

    pipeline.tick().unwrap();
    pipeline.tick().unwrap();

    let errors: Vec<_> = pipeline.errors().iter().collect();
    assert_eq!(errors.len(), 1);
    assert!(matches!(&errors[0].error, Error::Encode { path, .. } if path == &bad_output));
    assert!(errors[0].frame.is_some());
    assert_eq!(pipeline.errors().total(), 2);
    let world = pipeline.world_mut();
    let mut q = world.query::<&Frame>();
    assert_eq!(q.iter(world).count(), 0, "frames com erro são descartados");
}

/// A mesma falha de escrita interrompe o pipeline em `FailFast`.
#[test]
fn unwritable_output_makes_tick_fail_in_fail_fast() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input.png");
    create_test_png(&input, 4, 6);

    let mut pipeline = Pipeline::builder()
        .add_plugin(IoPlugin {
            input_path: input,
            output_path: tmp.path().join("pasta_inexistente").join("out.png"),
            ..Default::default()
        })
        .build();

    assert!(matches!(pipeline.tick(), Err(Error::Encode { .. })));
}

/// Em uso contínuo os frames não se acumulam: cada um é despawnado depois da saída.
#[test]
fn frames_are_despawned_after_output() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input.png");
    let output = tmp.path().join("output_despawn.png");
    create_test_png(&input, 4, 6);

    let mut pipeline = Pipeline::builder()
        .add_plugin(IoPlugin {
            input_path: input,
            output_path: output.clone(),
            ..Default::default()
        })
        .build();

    for _ in 0..3 {
        pipeline.tick().unwrap();
        let world = pipeline.world_mut();
        let mut q = world.query::<&Frame>();
        assert_eq!(q.iter(world).count(), 0, "frame deve sair no fim do tick");
    }
    assert_eq!(pipeline.world().resource::<PipelineState>().tick_count, 3);
    assert!(output.exists(), "a saída é escrita antes do despawn");
}

/// `keep_frames` preserva o comportamento antigo: um frame novo por tick.
#[test]
fn keep_frames_accumulates_one_frame_per_tick() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input.png");
    create_test_png(&input, 4, 6);

    let mut pipeline = Pipeline::builder()
        .add_plugin(IoPlugin {
            input_path: input,
            output_path: tmp.path().join("output_keep.png"),
            keep_frames: true,
            ..Default::default()
        })
        .build();

    for _ in 0..3 {
        pipeline.tick().unwrap();
    }
    let world = pipeline.world_mut();
    let mut q = world.query::<&Frame>();
    assert_eq!(q.iter(world).count(), 3);
}
