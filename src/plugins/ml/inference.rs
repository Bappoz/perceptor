//! Sistema de inferência de modelos ML sobre frames processados.
//!
//! # Fluxo esperado
//! 1. `MlPlugin` registra `ModelConfig` como recurso com o caminho do modelo.
//! 2. `inference_system` roda no `PostProcessStage`.
//! 3. Para cada frame pré-processado, normaliza o tensor e roda o modelo.
//! 4. Resultado é anexado como componente [`Prediction`] na entidade.
//!
//! # TODO: escolher backend
//! - **ONNX Runtime** (`ort`): melhor portabilidade, CPU/GPU.
//! - **tch-rs**: TorchScript nativo, ótimo para modelos PyTorch.
//! - Adicione a dependência escolhida em `Cargo.toml` com feature `ml`.

use bevy_ecs::prelude::*;
use tracing::warn;

use crate::core::frame::Frame;

/// Recurso que configura o modelo ML a ser usado.
#[derive(Resource, Debug, Default)]
pub struct ModelConfig {
    /// Caminho para o arquivo do modelo (`.onnx`, `.pt`, `.bin`…).
    pub model_path: String,
    /// Dimensões esperadas pelo modelo `[C, H, W]` (formato NCHW sem batch).
    pub input_shape: [usize; 3],
    /// Nomes das classes de saída (para classificação/detecção).
    pub class_names: Vec<String>,
}

/// Componente: resultado da inferência anexado ao frame após processamento.
#[derive(Component, Debug)]
pub struct Prediction {
    /// Scores por classe `[num_classes]`, valores em `[0.0, 1.0]`.
    pub scores: Vec<f32>,
    /// Índice da classe com maior score.
    pub class_id: usize,
    /// Confiança (max score).
    pub confidence: f32,
    // TODO: adicionar bounding boxes para detecção de objetos
    // pub bboxes: Vec<BoundingBox>,
}

/// Sistema ECS: roda inferência ML sobre frames disponíveis.
///
/// Registrado no `PostProcessStage` pelo [`MlPlugin`].
pub fn inference_system(
    query: Query<(Entity, &Frame)>,
    config: Option<Res<ModelConfig>>,
    mut commands: Commands,
) {
    let Some(config) = config else {
        warn!("inference_system: ModelConfig não encontrado no World — adicione como recurso");
        return;
    };

    if config.model_path.is_empty() {
        warn!("inference_system: ModelConfig.model_path não configurado, pulando");
        return;
    }

    for (entity, _frame) in query.iter() {
        // TODO: pré-processar frame (normalizar, redimensionar para input_shape)
        // TODO: rodar inferência com backend escolhido (ort / tch-rs)
        // TODO: pós-processar saída (softmax, NMS para detecção)

        let prediction = Prediction {
            scores: vec![0.0; config.class_names.len().max(1)],
            class_id: 0,
            confidence: 0.0,
        };

        commands.entity(entity).insert(prediction);
    }
}
