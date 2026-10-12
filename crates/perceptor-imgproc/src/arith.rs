//! Aritmética saturada e lógica bit a bit entre imagens, com máscara opcional.
//!
//! São os blocos de construção de composição (somar um brilho, misturar dois
//! quadros, recortar com uma máscara) e de diferença entre quadros
//! ([`absdiff`], a base da detecção de movimento).
//!
//! | Função | Resultado por canal |
//! |---|---|
//! | [`add`] | `a + b`, saturado |
//! | [`sub`] | `a − b`, saturado em `0` nos inteiros |
//! | [`absdiff`] | `\|a − b\|` |
//! | [`add_weighted`] | `α·a + β·b + γ`, arredondado e saturado |
//! | [`bitwise_and`], [`bitwise_or`], [`bitwise_xor`] | a operação, bit a bit |
//! | [`bitwise_not`] | `!a` |
//!
//! # Saturação
//!
//! Em `u8` e `u16` os resultados ficam em `[0, máx]` sem volta: `200 + 100`
//! dá `255`, `10 − 20` dá `0`. Em `f32` não há corte, e `sub` pode ser
//! negativo. As operações bit a bit só existem para `u8` e `u16`.
//!
//! # Máscara
//!
//! Uma máscara `L8` com as dimensões das imagens restringe a operação aos
//! pixels em que ela é **diferente de `0`**. Fora dela, o resultado é o
//! **primeiro operando** sem alteração — assim `add(fundo, brilho, máscara)`
//! soma só na região e deixa o resto como estava. A máscara vale para todos
//! os canais de um pixel.
//!
//! Operandos (e máscara) de dimensões diferentes dão
//! [`Error::ShapeMismatch`].

use perceptor_core::{Error, Image, ImageView, Pixel, Result, L8};

use crate::convert::Sample;

/// Amostra com aritmética saturada: `u8`, `u16` ou `f32`.
pub trait Arith: Sample {
    /// `self + other`, saturado nos inteiros.
    #[must_use]
    fn sat_add(self, other: Self) -> Self;
    /// `self − other`, saturado em `0` nos inteiros.
    #[must_use]
    fn sat_sub(self, other: Self) -> Self;
    /// `|self − other|`.
    #[must_use]
    fn sat_absdiff(self, other: Self) -> Self;
}

/// Amostra inteira, com operações bit a bit: `u8` ou `u16`.
pub trait Bits: Arith {
    /// `self & other`.
    #[must_use]
    fn bit_and(self, other: Self) -> Self;
    /// `self | other`.
    #[must_use]
    fn bit_or(self, other: Self) -> Self;
    /// `self ^ other`.
    #[must_use]
    fn bit_xor(self, other: Self) -> Self;
    /// `!self`.
    #[must_use]
    fn bit_not(self) -> Self;
}

macro_rules! impl_int_sample {
    ($t:ty) => {
        impl Arith for $t {
            fn sat_add(self, other: Self) -> Self {
                self.saturating_add(other)
            }
            fn sat_sub(self, other: Self) -> Self {
                self.saturating_sub(other)
            }
            fn sat_absdiff(self, other: Self) -> Self {
                self.abs_diff(other)
            }
        }
        impl Bits for $t {
            fn bit_and(self, other: Self) -> Self {
                self & other
            }
            fn bit_or(self, other: Self) -> Self {
                self | other
            }
            fn bit_xor(self, other: Self) -> Self {
                self ^ other
            }
            fn bit_not(self) -> Self {
                !self
            }
        }
    };
}
impl_int_sample!(u8);
impl_int_sample!(u16);

impl Arith for f32 {
    fn sat_add(self, other: Self) -> Self {
        self + other
    }
    fn sat_sub(self, other: Self) -> Self {
        self - other
    }
    fn sat_absdiff(self, other: Self) -> Self {
        (self - other).abs()
    }
}

/// Confere que `other` (e a máscara, se houver) tem as dimensões de `base`.
fn check_shapes(
    base: (usize, usize),
    other: Option<(usize, usize)>,
    mask: Option<&ImageView<'_, L8>>,
) -> Result<()> {
    let dims = other.into_iter().chain(mask.map(ImageView::dimensions));
    for actual in dims {
        if actual != base {
            return Err(Error::ShapeMismatch {
                expected: base,
                actual,
            });
        }
    }
    Ok(())
}

/// Aplica `f` canal a canal entre `a` e `b`, onde a máscara liga; fora dela,
/// copia `a`.
fn binary<P>(
    a: ImageView<'_, P>,
    b: ImageView<'_, P>,
    mask: Option<ImageView<'_, L8>>,
    f: impl Fn(P::Subpixel, P::Subpixel) -> P::Subpixel,
) -> Result<Image<P>>
where
    P: Pixel,
{
    check_shapes(a.dimensions(), Some(b.dimensions()), mask.as_ref())?;
    let mut output = Image::<P>::new(a.width(), a.height())?;
    for (y, ((dst, row_a), row_b)) in output.rows_mut().zip(a.rows()).zip(b.rows()).enumerate() {
        let mask_row = mask.as_ref().map(|m| m.row(y));
        for (x, ((out, pa), pb)) in dst.iter_mut().zip(row_a).zip(row_b).enumerate() {
            *out = *pa;
            if mask_row.is_none_or(|m| m[x].luma() != 0) {
                let channels = out.channels_mut().iter_mut();
                for ((slot, &va), &vb) in channels.zip(pa.channels()).zip(pb.channels()) {
                    *slot = f(va, vb);
                }
            }
        }
    }
    Ok(output)
}

/// `a + b` por canal, saturado.
///
/// # Errors
/// - [`Error::ShapeMismatch`] se `b` ou a máscara tiverem outras dimensões.
/// - [`Error::Allocation`] se a saída não puder ser alocada.
pub fn add<P>(
    a: ImageView<'_, P>,
    b: ImageView<'_, P>,
    mask: Option<ImageView<'_, L8>>,
) -> Result<Image<P>>
where
    P: Pixel,
    P::Subpixel: Arith,
{
    binary(a, b, mask, Arith::sat_add)
}

/// `a − b` por canal, saturado em `0` nos inteiros.
///
/// # Errors
/// Os mesmos de [`add`].
pub fn sub<P>(
    a: ImageView<'_, P>,
    b: ImageView<'_, P>,
    mask: Option<ImageView<'_, L8>>,
) -> Result<Image<P>>
where
    P: Pixel,
    P::Subpixel: Arith,
{
    binary(a, b, mask, Arith::sat_sub)
}

/// `|a − b|` por canal: simétrica e sem saturação.
///
/// # Errors
/// Os mesmos de [`add`].
pub fn absdiff<P>(
    a: ImageView<'_, P>,
    b: ImageView<'_, P>,
    mask: Option<ImageView<'_, L8>>,
) -> Result<Image<P>>
where
    P: Pixel,
    P::Subpixel: Arith,
{
    binary(a, b, mask, Arith::sat_absdiff)
}

/// `α·a + β·b + γ` por canal, arredondado e saturado no tipo da imagem.
///
/// Com `β = 1 − α` e `γ = 0` é a mistura linear de dois quadros.
///
/// # Errors
/// Os mesmos de [`add`].
pub fn add_weighted<P>(
    a: ImageView<'_, P>,
    alpha: f64,
    b: ImageView<'_, P>,
    beta: f64,
    gamma: f64,
    mask: Option<ImageView<'_, L8>>,
) -> Result<Image<P>>
where
    P: Pixel,
    P::Subpixel: Sample,
{
    binary(a, b, mask, |va, vb| {
        P::Subpixel::from_f64(alpha * va.to_f64() + beta * vb.to_f64() + gamma)
    })
}

/// `a & b` por canal.
///
/// # Errors
/// Os mesmos de [`add`].
pub fn bitwise_and<P>(
    a: ImageView<'_, P>,
    b: ImageView<'_, P>,
    mask: Option<ImageView<'_, L8>>,
) -> Result<Image<P>>
where
    P: Pixel,
    P::Subpixel: Bits,
{
    binary(a, b, mask, Bits::bit_and)
}

/// `a | b` por canal.
///
/// # Errors
/// Os mesmos de [`add`].
pub fn bitwise_or<P>(
    a: ImageView<'_, P>,
    b: ImageView<'_, P>,
    mask: Option<ImageView<'_, L8>>,
) -> Result<Image<P>>
where
    P: Pixel,
    P::Subpixel: Bits,
{
    binary(a, b, mask, Bits::bit_or)
}

/// `a ^ b` por canal.
///
/// # Errors
/// Os mesmos de [`add`].
pub fn bitwise_xor<P>(
    a: ImageView<'_, P>,
    b: ImageView<'_, P>,
    mask: Option<ImageView<'_, L8>>,
) -> Result<Image<P>>
where
    P: Pixel,
    P::Subpixel: Bits,
{
    binary(a, b, mask, Bits::bit_xor)
}

/// `!a` por canal: o complemento de cada bit (`255 − v` em 8 bits).
///
/// # Errors
/// - [`Error::ShapeMismatch`] se a máscara tiver outras dimensões.
/// - [`Error::Allocation`] se a saída não puder ser alocada.
pub fn bitwise_not<P>(a: ImageView<'_, P>, mask: Option<ImageView<'_, L8>>) -> Result<Image<P>>
where
    P: Pixel,
    P::Subpixel: Bits,
{
    // Unário: o segundo operando é o próprio `a` e é ignorado por `f`.
    binary(a, a, mask, |value, _| value.bit_not())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_samples_saturate_and_floats_do_not() {
        assert_eq!(200u8.sat_add(100), 255);
        assert_eq!(10u8.sat_sub(20), 0);
        assert_eq!(10u8.sat_absdiff(20), 10);
        assert_eq!(60_000u16.sat_add(10_000), 65_535);
        assert_eq!(0.75f32.sat_add(0.5), 1.25);
        assert_eq!(0.25f32.sat_sub(0.5), -0.25);
    }

    #[test]
    fn check_shapes_reports_the_first_offender() {
        let err = check_shapes((4, 3), Some((4, 2)), None).unwrap_err();
        assert!(matches!(
            err,
            Error::ShapeMismatch {
                expected: (4, 3),
                actual: (4, 2)
            }
        ));
        assert!(check_shapes((4, 3), Some((4, 3)), None).is_ok());
    }
}
