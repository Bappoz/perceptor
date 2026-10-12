//! Conversão YUV → RGB para os layouts que câmeras e decodificadores entregam.
//!
//! | Função | Layout | Subamostragem | Entrada |
//! |---|---|---|---|
//! | [`i420_to_rgb`] | planar | 4:2:0 | planos `Y`, `U` e `V` |
//! | [`nv12_to_rgb`] | semiplanar | 4:2:0 | plano `Y` e plano `UV` intercalado |
//! | [`yuyv_to_rgb`] | empacotado | 4:2:2 | bytes `Y0 U Y1 V` |
//!
//! As entradas são visões de bytes ([`ImageView<L8>`]) sobre o buffer
//! original, cada uma com o próprio stride: nada é copiado antes da conversão.
//!
//! # Matriz e faixa
//!
//! Um [`YuvSpec`] diz como o YUV foi codificado: a **matriz** (os pesos de
//! luminância, BT.601 ou BT.709) e a **faixa** (limitada: `Y` em `16..=235`
//! e croma em `16..=240`; completa: `0..=255`). Com `Kr` e `Kb` da matriz e
//! `Kg = 1 − Kr − Kb`, depois de normalizar a faixa:
//!
//! ```text
//! R = Y + 2(1 − Kr)·Cr
//! G = Y − [2Kb(1 − Kb)/Kg]·Cb − [2Kr(1 − Kr)/Kg]·Cr
//! B = Y + 2(1 − Kb)·Cb
//! ```
//!
//! O arquivo não diz qual especificação foi usada; escolher a errada desloca
//! as cores sem erro nenhum. O padrão é BT.601 de faixa limitada, o que
//! webcams entregam em YUYV.
//!
//! # Croma
//!
//! Cada amostra de croma vale para o bloco inteiro que ela cobre (2×2 em
//! 4:2:0, 2×1 em 4:2:2), sem interpolar.
//!
//! # Precisão
//!
//! A conta é em ponto fixo Q14, saturada em `0..=255`, e fica a no máximo um
//! nível de [`reference::yuv_to_rgb`], que calcula em `f64`.

// `cb`/`cr` e os coeficientes `cr_r`, `cb_g`, `cr_g`, `cb_b` são a notação
// padrão das matrizes YCbCr; nomes mais longos não ficam mais distintos.
#![allow(clippy::similar_names)]

use perceptor_core::{Error, Image, ImageView, Result, Rgb8, L8};

/// Pesos de luminância com que o YUV foi codificado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum YuvMatrix {
    /// ITU-R BT.601: SDTV, JPEG, a maioria das webcams.
    #[default]
    Bt601,
    /// ITU-R BT.709: HDTV e vídeo em alta definição.
    Bt709,
}

impl YuvMatrix {
    /// `(Kr, Kb)`: o peso do vermelho e o do azul na luminância.
    #[must_use]
    pub const fn kr_kb(self) -> (f64, f64) {
        match self {
            Self::Bt601 => (0.299, 0.114),
            Self::Bt709 => (0.2126, 0.0722),
        }
    }
}

/// Faixa de valores dos canais.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum YuvRange {
    /// Faixa limitada ("TV", "studio swing"): `Y` em `16..=235`, croma em
    /// `16..=240`. Valores fora dela são aceitos e saturam.
    #[default]
    Limited,
    /// Faixa completa ("PC", "full swing", JPEG): `0..=255`.
    Full,
}

/// Como o YUV foi codificado: matriz e faixa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct YuvSpec {
    /// Pesos de luminância.
    pub matrix: YuvMatrix,
    /// Faixa de valores.
    pub range: YuvRange,
}

impl YuvSpec {
    /// BT.601, faixa limitada. É o padrão.
    pub const BT601_LIMITED: Self = Self::new(YuvMatrix::Bt601, YuvRange::Limited);
    /// BT.601, faixa completa (JPEG/JFIF).
    pub const BT601_FULL: Self = Self::new(YuvMatrix::Bt601, YuvRange::Full);
    /// BT.709, faixa limitada.
    pub const BT709_LIMITED: Self = Self::new(YuvMatrix::Bt709, YuvRange::Limited);
    /// BT.709, faixa completa.
    pub const BT709_FULL: Self = Self::new(YuvMatrix::Bt709, YuvRange::Full);

    /// Combina uma matriz e uma faixa.
    #[must_use]
    pub const fn new(matrix: YuvMatrix, range: YuvRange) -> Self {
        Self { matrix, range }
    }

    /// Coeficientes em `f64`: `(offset de Y, ganho de Y, [Cr→R, Cb→G, Cr→G, Cb→B])`.
    fn coefficients(self) -> (f64, f64, [f64; 4]) {
        let (kr, kb) = self.matrix.kr_kb();
        let kg = 1.0 - kr - kb;
        let (y_offset, y_gain, chroma_gain) = match self.range {
            YuvRange::Limited => (16.0, 255.0 / 219.0, 255.0 / 224.0),
            YuvRange::Full => (0.0, 1.0, 1.0),
        };
        let chroma = [
            2.0 * (1.0 - kr),
            -2.0 * kb * (1.0 - kb) / kg,
            -2.0 * kr * (1.0 - kr) / kg,
            2.0 * (1.0 - kb),
        ];
        (y_offset, y_gain, chroma.map(|c| c * chroma_gain))
    }
}

/// Bits de fração dos coeficientes em ponto fixo.
const SHIFT: u32 = 14;
const HALF: i32 = 1 << (SHIFT - 1);

/// Coeficientes de um [`YuvSpec`] em Q14.
#[derive(Clone, Copy)]
struct Fixed {
    y_offset: i32,
    y_gain: i32,
    cr_r: i32,
    cb_g: i32,
    cr_g: i32,
    cb_b: i32,
}

impl Fixed {
    fn new(spec: YuvSpec) -> Self {
        let (y_offset, y_gain, [cr_r, cb_g, cr_g, cb_b]) = spec.coefficients();
        // O maior coeficiente é ≈ 2,3: em Q14 fica muito abaixo de `i32::MAX`.
        #[allow(clippy::cast_possible_truncation)]
        let q14 = |c: f64| (c * f64::from(1u32 << SHIFT)).round() as i32;
        #[allow(clippy::cast_possible_truncation)]
        let y_offset = y_offset as i32;
        Self {
            y_offset,
            y_gain: q14(y_gain),
            cr_r: q14(cr_r),
            cb_g: q14(cb_g),
            cr_g: q14(cr_g),
            cb_b: q14(cb_b),
        }
    }

    #[inline]
    fn rgb(self, y: u8, cb: u8, cr: u8) -> Rgb8 {
        let luma = self.y_gain * (i32::from(y) - self.y_offset) + HALF;
        let cb = i32::from(cb) - 128;
        let cr = i32::from(cr) - 128;
        Rgb8::new(
            saturate((luma + self.cr_r * cr) >> SHIFT),
            saturate((luma + self.cb_g * cb + self.cr_g * cr) >> SHIFT),
            saturate((luma + self.cb_b * cb) >> SHIFT),
        )
    }
}

fn saturate(value: i32) -> u8 {
    // Depois do `clamp` o valor está em `0..=255`.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let level = value.clamp(0, 255) as u8;
    level
}

fn require_even(width: usize, height: usize) -> Result<()> {
    if width.is_multiple_of(2) && height.is_multiple_of(2) {
        Ok(())
    } else {
        Err(Error::OddDimensions { width, height })
    }
}

fn require_shape(plane: ImageView<'_, L8>, expected: (usize, usize)) -> Result<()> {
    let actual = (plane.width(), plane.height());
    if actual == expected {
        Ok(())
    } else {
        Err(Error::ShapeMismatch { expected, actual })
    }
}

/// Converte I420 (YUV 4:2:0 planar) em RGB.
///
/// `u` e `v` têm metade da largura e metade da altura de `y`.
///
/// # Errors
/// - [`Error::OddDimensions`] se a largura ou a altura de `y` for ímpar.
/// - [`Error::ShapeMismatch`] se `u` ou `v` não tiver `(largura/2, altura/2)`.
/// - [`Error::Allocation`] se a saída não puder ser alocada.
pub fn i420_to_rgb(
    y: ImageView<'_, L8>,
    u: ImageView<'_, L8>,
    v: ImageView<'_, L8>,
    spec: YuvSpec,
) -> Result<Image<Rgb8>> {
    let (width, height) = (y.width(), y.height());
    require_even(width, height)?;
    require_shape(u, (width / 2, height / 2))?;
    require_shape(v, (width / 2, height / 2))?;

    let fixed = Fixed::new(spec);
    let mut output = Image::<Rgb8>::new(width, height)?;
    for (row, dst) in output.rows_mut().enumerate() {
        let (luma, cb, cr) = (y.row(row), u.row(row / 2), v.row(row / 2));
        for (x, out) in dst.iter_mut().enumerate() {
            *out = fixed.rgb(luma[x].luma(), cb[x / 2].luma(), cr[x / 2].luma());
        }
    }
    Ok(output)
}

/// Converte NV12 (YUV 4:2:0 semiplanar) em RGB.
///
/// `uv` é o plano de croma intercalado, `U V U V …`, visto como bytes: tem
/// a **mesma largura de `y`** (um par `U V` a cada dois pixels) e metade da
/// altura.
///
/// # Errors
/// - [`Error::OddDimensions`] se a largura ou a altura de `y` for ímpar.
/// - [`Error::ShapeMismatch`] se `uv` não tiver `(largura, altura/2)`.
/// - [`Error::Allocation`] se a saída não puder ser alocada.
pub fn nv12_to_rgb(
    y: ImageView<'_, L8>,
    uv: ImageView<'_, L8>,
    spec: YuvSpec,
) -> Result<Image<Rgb8>> {
    let (width, height) = (y.width(), y.height());
    require_even(width, height)?;
    require_shape(uv, (width, height / 2))?;

    let fixed = Fixed::new(spec);
    let mut output = Image::<Rgb8>::new(width, height)?;
    for (row, dst) in output.rows_mut().enumerate() {
        let (luma, chroma) = (y.row(row), uv.row(row / 2));
        for (x, out) in dst.iter_mut().enumerate() {
            let pair = x & !1;
            *out = fixed.rgb(luma[x].luma(), chroma[pair].luma(), chroma[pair + 1].luma());
        }
    }
    Ok(output)
}

/// Converte YUYV (YUV 4:2:2 empacotado, também chamado YUY2) em RGB.
///
/// `packed` é o buffer visto como bytes, `Y0 U Y1 V` a cada dois pixels: a
/// visão tem **o dobro da largura da imagem** e a mesma altura. A altura
/// pode ser ímpar; a largura da imagem não.
///
/// # Errors
/// - [`Error::BufferLength`] se a largura de `packed` for ímpar (não fecha
///   um número inteiro de pixels).
/// - [`Error::OddDimensions`] se a largura da imagem for ímpar.
/// - [`Error::Allocation`] se a saída não puder ser alocada.
pub fn yuyv_to_rgb(packed: ImageView<'_, L8>, spec: YuvSpec) -> Result<Image<Rgb8>> {
    let (bytes, height) = (packed.width(), packed.height());
    if !bytes.is_multiple_of(2) {
        return Err(Error::BufferLength {
            len: bytes,
            channels: 2,
        });
    }
    let width = bytes / 2;
    if !width.is_multiple_of(2) {
        return Err(Error::OddDimensions { width, height });
    }

    let fixed = Fixed::new(spec);
    let mut output = Image::<Rgb8>::new(width, height)?;
    for (dst, src) in output.rows_mut().zip(packed.rows()) {
        // As larguras foram validadas: não sobra resto em nenhuma das duas.
        let (pairs, _) = dst.as_chunks_mut::<2>();
        let (quads, _) = src.as_chunks::<4>();
        for ([first, second], [y0, blue, y1, red]) in pairs.iter_mut().zip(quads) {
            *first = fixed.rgb(y0.luma(), blue.luma(), red.luma());
            *second = fixed.rgb(y1.luma(), blue.luma(), red.luma());
        }
    }
    Ok(output)
}

/// Versão escalar de referência: a definição contra a qual a otimizada é testada.
pub mod reference {
    use super::YuvSpec;

    /// Converte um tripleto YUV em `[R, G, B]`, em `f64`, arredondando e
    /// saturando em `0..=255`.
    #[must_use]
    pub fn yuv_to_rgb(y: u8, cb: u8, cr: u8, spec: YuvSpec) -> [u8; 3] {
        let (y_offset, y_gain, [cr_r, cb_g, cr_g, cb_b]) = spec.coefficients();
        let luma = (f64::from(y) - y_offset) * y_gain;
        let cb = f64::from(cb) - 128.0;
        let cr = f64::from(cr) - 128.0;
        // Depois do `clamp` o valor está em `0..=255`.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let level = |v: f64| v.round().clamp(0.0, 255.0) as u8;
        [
            level(luma + cr_r * cr),
            level(luma + cb_g * cb + cr_g * cr),
            level(luma + cb_b * cb),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Todo `Y`, com croma de 3 em 3 níveis mais os extremos, nas quatro
    /// especificações: nunca mais de um nível de diferença para o `f64`.
    #[test]
    fn fixed_point_is_within_one_level_of_the_reference() {
        let chroma: Vec<u8> = (0..=255u8).step_by(3).collect();
        assert!(chroma.contains(&0) && chroma.contains(&255));
        for spec in [
            YuvSpec::BT601_LIMITED,
            YuvSpec::BT601_FULL,
            YuvSpec::BT709_LIMITED,
            YuvSpec::BT709_FULL,
        ] {
            let fixed = Fixed::new(spec);
            for y in 0..=255u8 {
                for &cb in &chroma {
                    for &cr in &chroma {
                        let fast = fixed.rgb(y, cb, cr);
                        let [r, g, b] = reference::yuv_to_rgb(y, cb, cr, spec);
                        let error = fast
                            .r()
                            .abs_diff(r)
                            .max(fast.g().abs_diff(g))
                            .max(fast.b().abs_diff(b));
                        assert!(
                            error <= 1,
                            "{spec:?} ({y}, {cb}, {cr}): {fast:?} vs {:?}",
                            [r, g, b]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn bt601_limited_coefficients_match_the_textbook_values() {
        let (offset, gain, [cr_r, cb_g, cr_g, cb_b]) = YuvSpec::BT601_LIMITED.coefficients();
        let close = |a: f64, b: f64| (a - b).abs() < 5e-4;
        assert!(close(offset, 16.0) && close(gain, 1.164));
        assert!(close(cr_r, 1.596) && close(cb_g, -0.392), "{cr_r} {cb_g}");
        assert!(close(cr_g, -0.813) && close(cb_b, 2.017), "{cr_g} {cb_b}");
    }
}
