//! Plugin customizado: inverte as cores de cada frame.
//!
//! ```text
//! cargo run --example custom_plugin
//! ```

use perceptor::pipeline::pipeline::PipelineBuilder;
use perceptor::prelude::*;

/// Kernel: função pura, sem ECS. Opera no lugar, sobre qualquer visão `L8`.
fn invert(mut image: ImageViewMut<'_, L8>) {
    for pixel in image.pixels_mut() {
        *pixel = L8::new(255 - pixel.luma());
    }
}

/// Marcador que evita reprocessar o mesmo frame nos ticks seguintes.
#[derive(Component)]
struct InvertTag;

/// Sistema: consulta os frames pendentes, chama o kernel e marca o resultado.
fn invert_system(
    mut query: Query<(Entity, &mut Frame), Without<InvertTag>>,
    mut commands: Commands,
) {
    for (entity, mut frame) in &mut query {
        // Acesso tipado: frames em outro formato são ignorados.
        let Ok(image) = frame.as_image_mut::<L8>() else {
            continue;
        };
        invert(image.view_mut());
        commands.entity(entity).insert(InvertTag);
    }
}

/// Plugin: registra o sistema no stage de processamento.
struct InvertPlugin;

impl Plugin for InvertPlugin {
    fn name(&self) -> &'static str {
        "InvertPlugin"
    }

    fn build(&self, builder: &mut PipelineBuilder) {
        builder.add_system(Stage::Process, invert_system);
    }
}

fn main() -> anyhow::Result<()> {
    let mut pipeline = Pipeline::builder().add_plugin(InvertPlugin).build()?;

    let data = Image::from_pixels(2, 1, &[L8::new(0), L8::new(255)])?;
    pipeline
        .world_mut()
        .spawn(Frame::new(FrameMeta::new(0, "sintético"), data));

    pipeline.tick()?;

    let world = pipeline.world_mut();
    let frame = world.query::<&Frame>().single(world)?;
    let pixels = frame.as_image::<L8>()?.as_pixels();
    assert_eq!(pixels, [L8::new(255), L8::new(0)]);
    println!("pixels invertidos: {pixels:?}");
    Ok(())
}
