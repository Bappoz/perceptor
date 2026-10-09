//! Plugin customizado: inverte as cores de cada frame.
//!
//! ```text
//! cargo run --example custom_plugin
//! ```

use perceptor::pipeline::{frame::FrameMeta, pipeline::PipelineBuilder};
use perceptor::prelude::*;

/// Kernel: função pura, sem ECS.
fn invert(input: &Array3<u8>) -> Array3<u8> {
    input.mapv(|v| 255 - v)
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
        frame.data = invert(&frame.data);
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
        builder.add_process_system(invert_system);
    }
}

fn main() -> anyhow::Result<()> {
    let mut pipeline = Pipeline::builder().add_plugin(InvertPlugin).build();

    let meta = FrameMeta {
        index: 0,
        timestamp_us: 0,
        source: "sintético".into(),
    };
    let data = Array3::from_shape_vec((1, 2, 1), vec![0u8, 255])?;
    pipeline.world_mut().spawn(Frame::new(meta, data));

    pipeline.tick()?;

    let world = pipeline.world_mut();
    let frame = world.query::<&Frame>().single(world)?;
    assert_eq!(frame.data.as_slice(), Some(&[255u8, 0][..]));
    println!("pixels invertidos: {:?}", frame.data.as_slice());
    Ok(())
}
