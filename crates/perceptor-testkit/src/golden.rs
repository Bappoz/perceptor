//! Imagens de referência ("golden") em PNG.
//!
//! O PNG é legível por qualquer visualizador, o que torna a falha de um
//! teste depurável a olho. Formatos suportados: `L8`, `La8`, `Rgb8`, `Rgba8`
//! e `L16`. Para kernels em ponto flutuante, quantize antes de comparar.
//!
//! # Regenerar
//!
//! ```text
//! PERCEPTOR_UPDATE_GOLDEN=1 cargo test -p <crate>
//! ```
//!
//! reescreve os arquivos em vez de compará-los. Revise o diff das imagens
//! antes de commitar: regenerar aceita qualquer saída como correta.

use std::path::Path;

use perceptor_core::{Image, La8, Pixel, Rgb8, Rgba8, L16, L8};

/// Variável de ambiente que liga a regeneração.
pub const UPDATE_VAR: &str = "PERCEPTOR_UPDATE_GOLDEN";

/// Formato de pixel que sabe ir e voltar de PNG.
pub trait GoldenPixel: Pixel {
    /// Grava a imagem como PNG.
    ///
    /// # Errors
    /// Descrição do erro de codificação ou de I/O.
    fn write_png(image: &Image<Self>, path: &Path) -> Result<(), String>;

    /// Lê um PNG, exigindo o tipo de cor do formato.
    ///
    /// # Errors
    /// Descrição do erro de I/O, de decodificação ou de tipo de cor.
    fn read_png(path: &Path) -> Result<Image<Self>, String>;
}

/// `true` se `PERCEPTOR_UPDATE_GOLDEN` estiver definida e não for `0`.
#[must_use]
pub fn update_requested() -> bool {
    std::env::var_os(UPDATE_VAR).is_some_and(|v| v != "0")
}

/// Compara `actual` com o PNG em `path`, ou o reescreve quando `update` é verdadeiro.
///
/// # Errors
/// Mensagem pronta para exibir: arquivo ausente, formato diferente, dimensões
/// diferentes ou amostras além de `tolerance`.
pub fn check<P: GoldenPixel>(
    path: &Path,
    actual: &Image<P>,
    tolerance: f64,
    update: bool,
) -> Result<(), String> {
    if update {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        return P::write_png(actual, path);
    }
    if !path.exists() {
        return Err(format!(
            "golden ausente: {}\n  gere com {UPDATE_VAR}=1 e revise a imagem antes de commitar",
            path.display()
        ));
    }
    let golden = P::read_png(path)?;
    match crate::compare::diff(&golden, actual, tolerance) {
        Err(shape) => Err(format!("{}: {shape} (golden contra atual)", path.display())),
        Ok(d) if d.differing > 0 => Err(format!(
            "{}: {}\n  se a mudança for intencional, regenere com {UPDATE_VAR}=1",
            path.display(),
            crate::compare::report(&d, tolerance)
        )),
        Ok(_) => Ok(()),
    }
}

macro_rules! golden_pixel {
    ($($ty:ty => $color:ident, $into:ident);* $(;)?) => {
        $(impl GoldenPixel for $ty {
            fn write_png(image: &Image<Self>, path: &Path) -> Result<(), String> {
                let buffer = image.to_image_buffer().map_err(|e| e.to_string())?;
                buffer
                    .save_with_format(path, ::image::ImageFormat::Png)
                    .map_err(|e| format!("{}: {e}", path.display()))
            }

            fn read_png(path: &Path) -> Result<Image<Self>, String> {
                let decoded = ::image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
                if decoded.color() != ::image::ColorType::$color {
                    return Err(format!(
                        "{}: tipo de cor {:?}, esperado {:?}",
                        path.display(),
                        decoded.color(),
                        ::image::ColorType::$color
                    ));
                }
                Image::from_image_buffer(&decoded.$into()).map_err(|e| e.to_string())
            }
        })*
    };
}

golden_pixel! {
    L8 => L8, into_luma8;
    La8 => La8, into_luma_alpha8;
    Rgb8 => Rgb8, into_rgb8;
    Rgba8 => Rgba8, into_rgba8;
    L16 => L16, into_luma16;
}

#[doc(hidden)]
#[track_caller]
pub fn __check_golden<P: GoldenPixel>(path: &Path, actual: &Image<P>, tolerance: f64) {
    if let Err(message) = check(path, actual, tolerance, update_requested()) {
        panic!("assert_golden! falhou: {message}");
    }
}

/// Afirma que a imagem é igual ao PNG `tests/golden/<nome>` da crate que chama.
///
/// ```ignore
/// assert_golden!(sobel(&input), "sobel_gradient.png");
/// assert_golden!(blur(&input), "blur_noise.png", tolerance = 1.0);
/// ```
#[macro_export]
macro_rules! assert_golden {
    ($image:expr, $name:literal $(,)?) => {
        $crate::assert_golden!($image, $name, tolerance = 0.0)
    };
    ($image:expr, $name:literal, tolerance = $tolerance:expr $(,)?) => {
        $crate::__check_golden(
            ::std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/", $name)),
            &$image,
            $tolerance,
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns;

    fn path(dir: &tempfile::TempDir, name: &str) -> std::path::PathBuf {
        dir.path().join(name)
    }

    #[test]
    fn missing_golden_explains_how_to_create_it() {
        let dir = tempfile::tempdir().unwrap();
        let err = check(
            &path(&dir, "x.png"),
            &patterns::gradient::<L8>(4, 4),
            0.0,
            false,
        )
        .unwrap_err();
        assert!(err.contains(UPDATE_VAR), "{err}");
        assert!(err.contains("x.png"), "{err}");
    }

    #[test]
    fn update_writes_then_compare_passes() {
        let dir = tempfile::tempdir().unwrap();
        let file = path(&dir, "g.png");
        let image = patterns::noise::<Rgb8>(9, 7, 3);
        check(&file, &image, 0.0, true).unwrap();
        assert!(file.exists());
        check(&file, &image, 0.0, false).unwrap();
    }

    #[test]
    fn every_supported_format_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        check(
            &path(&dir, "a.png"),
            &patterns::noise::<L8>(5, 5, 1),
            0.0,
            true,
        )
        .unwrap();
        check(
            &path(&dir, "b.png"),
            &patterns::noise::<La8>(5, 5, 1),
            0.0,
            true,
        )
        .unwrap();
        check(
            &path(&dir, "c.png"),
            &patterns::noise::<Rgba8>(5, 5, 1),
            0.0,
            true,
        )
        .unwrap();
        check(
            &path(&dir, "d.png"),
            &patterns::noise::<L16>(5, 5, 1),
            0.0,
            true,
        )
        .unwrap();
        check(
            &path(&dir, "a.png"),
            &patterns::noise::<L8>(5, 5, 1),
            0.0,
            false,
        )
        .unwrap();
        check(
            &path(&dir, "b.png"),
            &patterns::noise::<La8>(5, 5, 1),
            0.0,
            false,
        )
        .unwrap();
        check(
            &path(&dir, "c.png"),
            &patterns::noise::<Rgba8>(5, 5, 1),
            0.0,
            false,
        )
        .unwrap();
        check(
            &path(&dir, "d.png"),
            &patterns::noise::<L16>(5, 5, 1),
            0.0,
            false,
        )
        .unwrap();
    }

    #[test]
    fn divergence_reports_the_diff() {
        let dir = tempfile::tempdir().unwrap();
        let file = path(&dir, "g.png");
        let golden = patterns::gradient::<L8>(4, 1);
        check(&file, &golden, 0.0, true).unwrap();
        let mut actual = golden.clone();
        actual[(2, 0)] = L8::new(100);
        let err = check(&file, &actual, 0.0, false).unwrap_err();
        assert!(err.contains("x=2, y=0, c=0"), "{err}");
        assert!(err.contains(UPDATE_VAR), "{err}");
        check(&file, &actual, 70.0, false).unwrap();
    }

    #[test]
    fn shape_and_color_type_mismatches_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let file = path(&dir, "g.png");
        check(&file, &patterns::gradient::<L8>(4, 4), 0.0, true).unwrap();
        let err = check(&file, &patterns::gradient::<L8>(2, 2), 0.0, false).unwrap_err();
        assert!(err.contains("dimensões diferentes"), "{err}");
        let err = check(&file, &patterns::gradient::<Rgb8>(4, 4), 0.0, false).unwrap_err();
        assert!(err.contains("tipo de cor"), "{err}");
    }
}
