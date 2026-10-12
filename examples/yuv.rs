//! YUV → RGB sobre o buffer original, sem cópia.
//!
//! ```text
//! cargo run --example yuv
//! ```

use perceptor::core::Result;
use perceptor::imgproc::yuv::{nv12_to_rgb, yuyv_to_rgb, YuvSpec};
use perceptor::prelude::*;

fn main() -> Result<()> {
    // Um frame NV12 de 4×2 como um decodificador entrega: um buffer só, com
    // o plano Y (2 linhas) seguido do plano UV (1 linha), e 4 bytes de
    // preenchimento no fim de cada linha.
    let (width, height, stride) = (4, 2, 8);
    #[rustfmt::skip]
    let buffer: [u8; 24] = [
        // Y: metade esquerda escura, metade direita clara
         41,  41, 235, 235,   0, 0, 0, 0,
         41,  41, 235, 235,   0, 0, 0, 0,
        // UV: azul à esquerda, neutro à direita
        240, 110, 128, 128,   0, 0, 0, 0,
    ];

    // As visões apontam para dentro do buffer; o stride pula o preenchimento.
    let (luma, chroma) = buffer.split_at(height * stride);
    let y = ImageView::<L8>::new(luma, width, height, stride)?;
    // O plano UV é visto como bytes: mesma largura do Y, metade da altura.
    let uv = ImageView::<L8>::new(chroma, width, height / 2, stride)?;

    let rgb = nv12_to_rgb(y, uv, YuvSpec::BT601_LIMITED)?;
    println!("NV12 → {:?}", rgb.row(0));
    assert_eq!(rgb.row(0)[0], Rgb8::new(0, 0, 255));
    assert_eq!(rgb.row(1)[3], Rgb8::new(255, 255, 255));

    // A mesma imagem lida com a especificação errada: nenhum erro, só cores
    // deslocadas. O arquivo não diz qual é a certa.
    let wrong = nv12_to_rgb(y, uv, YuvSpec::BT709_FULL)?;
    println!("…como BT.709 completa: {:?}", wrong.row(0)[0]);
    assert_ne!(wrong.row(0)[0], rgb.row(0)[0]);

    // YUYV (webcams): `Y0 U Y1 V` a cada dois pixels. A visão de bytes tem o
    // dobro da largura da imagem.
    let packed: [u8; 8] = [41, 240, 41, 110, 235, 128, 235, 128];
    let view = ImageView::<L8>::new(&packed, 8, 1, 8)?;
    let from_yuyv = yuyv_to_rgb(view, YuvSpec::default())?;
    assert_eq!(from_yuyv.row(0), rgb.row(0));

    // Largura ímpar não tem como repartir o croma: erro, não pânico.
    let odd = Image::<L8>::new(3, 2)?;
    let err = nv12_to_rgb(odd.view(), uv, YuvSpec::default()).unwrap_err();
    println!("erro: {err}");
    Ok(())
}
