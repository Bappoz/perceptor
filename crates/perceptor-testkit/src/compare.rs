//! Comparação numérica entre imagens.

use std::fmt;

use perceptor_core::{Image, Pixel};

use crate::sample::{peak, to_f64};

/// Quantas amostras divergentes entram no relatório de [`Diff`].
const REPORTED: usize = 8;

/// Imagens com dimensões diferentes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeMismatch {
    /// `(largura, altura)` da esquerda.
    pub left: (usize, usize),
    /// `(largura, altura)` da direita.
    pub right: (usize, usize),
}

impl fmt::Display for ShapeMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "dimensões diferentes: {}x{} contra {}x{}",
            self.left.0, self.left.1, self.right.0, self.right.1
        )
    }
}

impl std::error::Error for ShapeMismatch {}

/// Uma amostra (canal de um pixel) divergente.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mismatch {
    /// Coluna do pixel.
    pub x: usize,
    /// Linha do pixel.
    pub y: usize,
    /// Índice do canal dentro do pixel.
    pub channel: usize,
    /// Valor à esquerda, na unidade nativa do formato.
    pub left: f64,
    /// Valor à direita, na unidade nativa do formato.
    pub right: f64,
}

/// Resultado da comparação de duas imagens de mesmas dimensões.
#[derive(Debug, Clone, PartialEq)]
pub struct Diff {
    /// Maior `|esquerda − direita|` entre todas as amostras.
    pub max_abs: f64,
    /// Erro quadrático médio.
    pub mse: f64,
    /// Fundo de escala do formato (`255`, `65535` ou `1.0`).
    pub peak: f64,
    /// Amostras cuja diferença excede a tolerância.
    pub differing: usize,
    /// Total de amostras comparadas.
    pub total: usize,
    /// As primeiras amostras que excedem a tolerância, em ordem de linha.
    pub first: Vec<Mismatch>,
}

impl Diff {
    /// PSNR em dB; infinito quando as imagens são idênticas.
    #[must_use]
    pub fn psnr(&self) -> f64 {
        if self.mse == 0.0 {
            f64::INFINITY
        } else {
            10.0 * (self.peak * self.peak / self.mse).log10()
        }
    }
}

/// Compara `left` e `right` amostra a amostra.
///
/// Uma amostra "diverge" quando `|esquerda − direita| > tolerance`; a
/// tolerância é na unidade nativa do formato.
///
/// # Errors
/// [`ShapeMismatch`] se as dimensões forem diferentes.
pub fn diff<P: Pixel>(
    left: &Image<P>,
    right: &Image<P>,
    tolerance: f64,
) -> Result<Diff, ShapeMismatch> {
    if left.dimensions() != right.dimensions() {
        return Err(ShapeMismatch {
            left: left.dimensions(),
            right: right.dimensions(),
        });
    }
    let channels = P::FORMAT.channels();
    let width = left.width();

    let mut result = Diff {
        max_abs: 0.0,
        mse: 0.0,
        peak: peak::<P::Subpixel>(),
        differing: 0,
        total: left.as_subpixels().len(),
        first: Vec::new(),
    };
    let mut squared = 0.0;
    let samples = left.as_subpixels().iter().zip(right.as_subpixels());
    for (index, (&l, &r)) in samples.enumerate() {
        let (l, r) = (to_f64(l), to_f64(r));
        let delta = (l - r).abs();
        result.max_abs = result.max_abs.max(delta);
        squared += delta * delta;
        if delta > tolerance {
            result.differing += 1;
            if result.first.len() < REPORTED {
                let pixel = index / channels;
                result.first.push(Mismatch {
                    x: pixel % width,
                    y: pixel / width,
                    channel: index % channels,
                    left: l,
                    right: r,
                });
            }
        }
    }
    #[allow(clippy::cast_precision_loss)] // contagem de amostras bem abaixo de 2^52
    {
        result.mse = squared / result.total as f64;
    }
    Ok(result)
}

/// Maior diferença absoluta entre amostras.
///
/// # Errors
/// [`ShapeMismatch`] se as dimensões forem diferentes.
pub fn max_abs_diff<P: Pixel>(left: &Image<P>, right: &Image<P>) -> Result<f64, ShapeMismatch> {
    diff(left, right, 0.0).map(|d| d.max_abs)
}

/// PSNR em dB; infinito para imagens idênticas.
///
/// # Errors
/// [`ShapeMismatch`] se as dimensões forem diferentes.
pub fn psnr<P: Pixel>(left: &Image<P>, right: &Image<P>) -> Result<f64, ShapeMismatch> {
    diff(left, right, 0.0).map(|d| d.psnr())
}

/// Texto do relatório de divergência, usado pelas macros de asserção.
#[must_use]
pub fn report(diff: &Diff, tolerance: f64) -> String {
    use fmt::Write as _;

    let mut text = format!(
        "{} de {} amostras divergem (tolerância {tolerance}): max |Δ| = {}, PSNR = {:.2} dB",
        diff.differing,
        diff.total,
        diff.max_abs,
        diff.psnr()
    );
    for m in &diff.first {
        // `write!` em `String` não falha.
        let _ = write!(
            text,
            "\n  (x={}, y={}, c={}): esquerda = {}, direita = {}",
            m.x, m.y, m.channel, m.left, m.right
        );
    }
    if diff.differing > diff.first.len() {
        let _ = write!(text, "\n  … e mais {}", diff.differing - diff.first.len());
    }
    text
}

/// Panica com o relatório se as imagens divergirem além de `tolerance`.
///
/// Usada por [`assert_images_eq!`](crate::assert_images_eq); chame a macro.
///
/// # Panics
/// Quando as dimensões diferem ou alguma amostra excede a tolerância.
#[doc(hidden)]
#[track_caller]
pub fn __assert_images_eq<P: Pixel>(left: &Image<P>, right: &Image<P>, tolerance: f64) {
    match diff(left, right, tolerance) {
        Err(shape) => panic!("assert_images_eq! falhou: {shape}"),
        Ok(d) if d.differing > 0 => {
            panic!("assert_images_eq! falhou: {}", report(&d, tolerance));
        }
        Ok(_) => {}
    }
}

/// Afirma que duas `Image<P>` são iguais, com relatório legível na falha.
///
/// ```
/// use perceptor_core::{Image, L8};
/// use perceptor_testkit::assert_images_eq;
///
/// let a = Image::filled(4, 4, L8::new(10)).unwrap();
/// let b = Image::filled(4, 4, L8::new(11)).unwrap();
/// assert_images_eq!(a, a);
/// assert_images_eq!(a, b, tolerance = 1.0); // até 1 nível de diferença
/// ```
///
/// Na falha a mensagem traz a contagem de amostras divergentes, `max |Δ|`,
/// o PSNR e as primeiras divergências com coordenadas `(x, y, canal)`.
#[macro_export]
macro_rules! assert_images_eq {
    ($left:expr, $right:expr $(,)?) => {
        $crate::compare::__assert_images_eq(&$left, &$right, 0.0)
    };
    ($left:expr, $right:expr, tolerance = $tolerance:expr $(,)?) => {
        $crate::compare::__assert_images_eq(&$left, &$right, $tolerance)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use perceptor_core::{Rgb8, L8, LF32};

    fn gray(width: usize, height: usize, value: u8) -> Image<L8> {
        Image::filled(width, height, L8::new(value)).unwrap()
    }

    #[test]
    fn identical_images_have_zero_error_and_infinite_psnr() {
        let a = gray(4, 3, 9);
        let d = diff(&a, &a.clone(), 0.0).unwrap();
        assert_eq!(d.max_abs, 0.0);
        assert_eq!(d.mse, 0.0);
        assert_eq!(d.differing, 0);
        assert_eq!(d.total, 12);
        assert!(d.psnr().is_infinite());
    }

    #[test]
    fn single_sample_difference_is_measured() {
        let a = gray(4, 4, 100);
        let mut b = a.clone();
        b[(1, 2)] = L8::new(105);
        let d = diff(&a, &b, 0.0).unwrap();
        assert_eq!(d.max_abs, 5.0);
        assert_eq!(d.differing, 1);
        assert!((d.mse - 25.0 / 16.0).abs() < 1e-12);
        let expected = 10.0 * (255.0_f64 * 255.0 / (25.0 / 16.0)).log10();
        assert!((d.psnr() - expected).abs() < 1e-9);
        assert_eq!(
            d.first,
            [Mismatch {
                x: 1,
                y: 2,
                channel: 0,
                left: 100.0,
                right: 105.0
            }]
        );
    }

    #[test]
    fn mismatch_reports_pixel_and_channel() {
        let a = Image::filled(3, 2, Rgb8::new(0, 0, 0)).unwrap();
        let mut b = a.clone();
        b[(2, 1)] = Rgb8::new(0, 7, 0);
        let d = diff(&a, &b, 0.0).unwrap();
        assert_eq!(d.first[0].x, 2);
        assert_eq!(d.first[0].y, 1);
        assert_eq!(d.first[0].channel, 1);
    }

    #[test]
    fn tolerance_is_inclusive() {
        let (a, b) = (gray(2, 2, 10), gray(2, 2, 15));
        assert_eq!(diff(&a, &b, 5.0).unwrap().differing, 0);
        assert_eq!(diff(&a, &b, 4.0).unwrap().differing, 4);
    }

    #[test]
    fn float_images_use_the_unit_range_for_psnr() {
        let a = Image::filled(2, 2, LF32::new(0.0)).unwrap();
        let b = Image::filled(2, 2, LF32::new(0.5)).unwrap();
        let d = diff(&a, &b, 0.0).unwrap();
        assert_eq!(d.max_abs, 0.5);
        assert!((d.psnr() - 10.0 * (1.0_f64 / 0.25).log10()).abs() < 1e-9);
    }

    #[test]
    fn shape_mismatch_is_an_error() {
        let err = diff(&gray(3, 2, 0), &gray(2, 3, 0), 0.0).unwrap_err();
        assert_eq!(
            err,
            ShapeMismatch {
                left: (3, 2),
                right: (2, 3)
            }
        );
        assert!(max_abs_diff(&gray(3, 2, 0), &gray(2, 3, 0)).is_err());
    }

    #[test]
    fn report_is_capped_and_counts_the_rest() {
        let (a, b) = (gray(5, 5, 0), gray(5, 5, 1));
        let text = report(&diff(&a, &b, 0.0).unwrap(), 0.0);
        assert!(text.starts_with("25 de 25 amostras divergem"), "{text}");
        assert_eq!(text.matches("esquerda =").count(), REPORTED);
        assert!(text.ends_with("… e mais 17"), "{text}");
    }

    #[test]
    fn assert_images_eq_passes_within_tolerance() {
        assert_images_eq!(gray(2, 2, 10), gray(2, 2, 12), tolerance = 2.0);
    }

    #[test]
    #[should_panic(expected = "(x=0, y=0, c=0): esquerda = 10, direita = 12")]
    fn assert_images_eq_fails_with_a_readable_diff() {
        assert_images_eq!(gray(2, 2, 10), gray(2, 2, 12));
    }

    #[test]
    #[should_panic(expected = "dimensões diferentes: 2x2 contra 3x3")]
    fn assert_images_eq_fails_on_shape() {
        assert_images_eq!(gray(2, 2, 0), gray(3, 3, 0));
    }
}
