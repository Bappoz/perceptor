//! Extrapolação de pixels além das bordas da imagem.
//!
//! Filtros espaciais precisam de vizinhos que não existem nas bordas. Um
//! [`BorderMode`] define de onde esses valores vêm. Os nomes e a convenção de
//! cada modo seguem os do `OpenCV`, o que permite comparar resultados.

use crate::{ImageView, Pixel};

/// Regra de extrapolação além das bordas.
///
/// Nos diagramas, `abcdefgh` é a imagem e as letras fora das barras são os
/// valores extrapolados.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BorderMode<P> {
    /// Valor fixo: `iiiiii|abcdefgh|iiiiii`.
    Constant(P),
    /// Repete o pixel da borda: `aaaaaa|abcdefgh|hhhhhh`.
    Replicate,
    /// Espelha incluindo a borda: `fedcba|abcdefgh|hgfedc`.
    Reflect,
    /// Espelha sem repetir a borda: `gfedcb|abcdefgh|gfedcb`.
    Reflect101,
    /// Repete a imagem periodicamente: `cdefgh|abcdefgh|abcdef`.
    Wrap,
}

impl<P> BorderMode<P> {
    /// Mapeia um índice possivelmente fora de `0..len` para um índice válido.
    ///
    /// Devolve `None` quando o valor não vem da imagem: em
    /// [`BorderMode::Constant`] fora dos limites, ou quando `len == 0`.
    /// Para os demais modos o resultado é sempre `Some(i)` com `i < len`,
    /// para qualquer `index`.
    #[must_use]
    pub fn map_index(&self, index: isize, len: usize) -> Option<usize> {
        if len == 0 {
            return None;
        }
        if let Some(inside) = usize::try_from(index).ok().filter(|&i| i < len) {
            return Some(inside);
        }

        // i128 comporta `2 · len` para qualquer `len: usize` e qualquer `index: isize`.
        let i = i128::try_from(index).ok()?;
        let n = i128::try_from(len).ok()?;
        let mapped = match self {
            Self::Constant(_) => return None,
            Self::Replicate => i.clamp(0, n - 1),
            Self::Wrap => i.rem_euclid(n),
            Self::Reflect => {
                let m = i.rem_euclid(2 * n);
                if m < n {
                    m
                } else {
                    2 * n - 1 - m
                }
            }
            Self::Reflect101 if n == 1 => 0,
            Self::Reflect101 => {
                let period = 2 * n - 2;
                let m = i.rem_euclid(period);
                if m < n {
                    m
                } else {
                    period - m
                }
            }
        };
        usize::try_from(mapped).ok()
    }
}

impl<P: Pixel> ImageView<'_, P> {
    /// Pixel em `(x, y)`, extrapolado por `border` quando fora da imagem.
    #[must_use]
    pub fn get_bordered(&self, x: isize, y: isize, border: BorderMode<P>) -> P {
        let mapped = border
            .map_index(x, self.width())
            .zip(border.map_index(y, self.height()));
        match (mapped, border) {
            (Some((x, y)), _) => self.row(y)[x],
            (None, BorderMode::Constant(value)) => value,
            // Só `Constant` devolve `None` em um eixo não vazio, e visões nunca são vazias.
            (None, _) => P::default(),
        }
    }

    /// Escreve em `out` a linha `y` com `pad` pixels extrapolados de cada lado.
    ///
    /// `y` também pode estar fora da imagem. `out` é limpo e termina com
    /// `width + 2 · pad` pixels; reutilize o mesmo `Vec` entre chamadas para
    /// não alocar por linha.
    pub fn padded_row(&self, y: isize, pad: usize, border: BorderMode<P>, out: &mut Vec<P>) {
        let width = self.width();
        out.clear();

        let Some(row) = border.map_index(y, self.height()).map(|y| self.row(y)) else {
            let fill = match border {
                BorderMode::Constant(value) => value,
                _ => P::default(),
            };
            out.resize(width + 2 * pad, fill);
            return;
        };

        let at = |x: isize| match (border.map_index(x, width), border) {
            (Some(x), _) => row[x],
            (None, BorderMode::Constant(value)) => value,
            (None, _) => P::default(),
        };
        // `pad` e `width` cabem em `isize`: ambos são limitados pelo tamanho de um buffer.
        let pad_signed = isize::try_from(pad).unwrap_or(isize::MAX);
        let width_signed = isize::try_from(width).unwrap_or(isize::MAX);

        out.extend((-pad_signed..0).map(at));
        out.extend_from_slice(row);
        out.extend((width_signed..width_signed.saturating_add(pad_signed)).map(at));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Image, L8};
    use proptest::prelude::*;

    const SOURCE: &[u8; 8] = b"abcdefgh";

    /// Estende "abcdefgh" 6 posições para cada lado, como nas tabelas do `OpenCV`.
    fn extend(border: BorderMode<L8>) -> String {
        (-6isize..14)
            .map(|i| match border.map_index(i, SOURCE.len()) {
                Some(mapped) => char::from(SOURCE[mapped]),
                None => 'i',
            })
            .collect()
    }

    #[test]
    fn index_mapping_matches_the_opencv_convention_table() {
        assert_eq!(
            extend(BorderMode::Constant(L8::new(0))),
            "iiiiiiabcdefghiiiiii"
        );
        assert_eq!(extend(BorderMode::Replicate), "aaaaaaabcdefghhhhhhh");
        assert_eq!(extend(BorderMode::Reflect), "fedcbaabcdefghhgfedc");
        assert_eq!(extend(BorderMode::Reflect101), "gfedcbabcdefghgfedcb");
        assert_eq!(extend(BorderMode::Wrap), "cdefghabcdefghabcdef");
    }

    #[test]
    fn reflection_keeps_bouncing_far_from_the_image() {
        // n = 3: "abc" → Reflect tem período 6, Reflect101 tem período 4.
        let reflect: Vec<_> = (-7isize..10)
            .map(|i| BorderMode::<L8>::Reflect.map_index(i, 3).unwrap())
            .collect();
        assert_eq!(reflect, [0, 0, 1, 2, 2, 1, 0, 0, 1, 2, 2, 1, 0, 0, 1, 2, 2]);
        let reflect101: Vec<_> = (-5isize..8)
            .map(|i| BorderMode::<L8>::Reflect101.map_index(i, 3).unwrap())
            .collect();
        assert_eq!(reflect101, [1, 0, 1, 2, 1, 0, 1, 2, 1, 0, 1, 2, 1]);
    }

    #[test]
    fn single_element_and_empty_axes_are_handled() {
        for border in [
            BorderMode::<L8>::Replicate,
            BorderMode::Reflect,
            BorderMode::Reflect101,
            BorderMode::Wrap,
        ] {
            assert_eq!(border.map_index(-5, 1), Some(0));
            assert_eq!(border.map_index(9, 1), Some(0));
            assert_eq!(border.map_index(0, 0), None);
        }
        assert_eq!(BorderMode::Constant(L8::new(1)).map_index(0, 1), Some(0));
        assert_eq!(BorderMode::Constant(L8::new(1)).map_index(1, 1), None);
    }

    #[test]
    fn get_bordered_reads_inside_and_extrapolates_outside() {
        // 0 1 2
        // 3 4 5
        let img = Image::from_fn(3, 2, |x, y| L8::new(u8::try_from(y * 3 + x).unwrap())).unwrap();
        let view = img.view();
        assert_eq!(view.get_bordered(1, 1, BorderMode::Replicate), L8::new(4));
        assert_eq!(view.get_bordered(-1, -1, BorderMode::Replicate), L8::new(0));
        assert_eq!(view.get_bordered(5, 0, BorderMode::Replicate), L8::new(2));
        assert_eq!(view.get_bordered(3, 0, BorderMode::Wrap), L8::new(0));
        assert_eq!(view.get_bordered(0, -1, BorderMode::Wrap), L8::new(3));
        assert_eq!(view.get_bordered(-1, 0, BorderMode::Reflect101), L8::new(1));
        assert_eq!(view.get_bordered(3, 1, BorderMode::Reflect), L8::new(5));
        let fill = L8::new(99);
        assert_eq!(view.get_bordered(-1, 0, BorderMode::Constant(fill)), fill);
        assert_eq!(view.get_bordered(0, 2, BorderMode::Constant(fill)), fill);
        assert_eq!(
            view.get_bordered(2, 1, BorderMode::Constant(fill)),
            L8::new(5)
        );
    }

    #[test]
    fn padded_row_extends_both_sides_and_reuses_the_buffer() {
        let img = Image::from_fn(3, 2, |x, y| L8::new(u8::try_from(y * 3 + x).unwrap())).unwrap();
        let view = img.view();
        let lumas = |row: &[L8]| row.iter().map(|p| p.luma()).collect::<Vec<_>>();
        let mut row = Vec::new();

        view.padded_row(0, 2, BorderMode::Reflect101, &mut row);
        assert_eq!(lumas(&row), [2, 1, 0, 1, 2, 1, 0]);

        let capacity = row.capacity();
        view.padded_row(1, 2, BorderMode::Replicate, &mut row);
        assert_eq!(lumas(&row), [3, 3, 3, 4, 5, 5, 5]);
        assert_eq!(row.capacity(), capacity, "o buffer deve ser reutilizado");

        // Linha acima da imagem: espelhada na vertical, estendida na horizontal.
        view.padded_row(-1, 1, BorderMode::Reflect, &mut row);
        assert_eq!(lumas(&row), [0, 0, 1, 2, 2]);

        // Com borda constante, uma linha fora da imagem é toda constante.
        view.padded_row(2, 1, BorderMode::Constant(L8::new(9)), &mut row);
        assert_eq!(lumas(&row), [9, 9, 9, 9, 9]);
        view.padded_row(1, 1, BorderMode::Constant(L8::new(9)), &mut row);
        assert_eq!(lumas(&row), [9, 3, 4, 5, 9]);

        view.padded_row(0, 0, BorderMode::Wrap, &mut row);
        assert_eq!(lumas(&row), [0, 1, 2]);
    }

    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)] // centenas de casos: lento demais sob interpretação
        fn mapped_index_is_always_in_bounds(index in any::<isize>(), len in 1usize..2000) {
            for border in [
                BorderMode::<L8>::Replicate,
                BorderMode::Reflect,
                BorderMode::Reflect101,
                BorderMode::Wrap,
            ] {
                let mapped = border.map_index(index, len);
                prop_assert!(mapped.is_some_and(|m| m < len), "{border:?}: {index} → {mapped:?}");
            }
            if let Some(mapped) = BorderMode::Constant(L8::new(0)).map_index(index, len) {
                prop_assert_eq!(isize::try_from(mapped).unwrap(), index);
            }
        }

        #[test]
        #[cfg_attr(miri, ignore)]
        fn indices_inside_the_axis_map_to_themselves(len in 1usize..500, seed in any::<usize>()) {
            let index = seed % len;
            for border in [
                BorderMode::Constant(L8::new(0)),
                BorderMode::Replicate,
                BorderMode::Reflect,
                BorderMode::Reflect101,
                BorderMode::Wrap,
            ] {
                prop_assert_eq!(border.map_index(isize::try_from(index).unwrap(), len), Some(index));
            }
        }

        #[test]
        #[cfg_attr(miri, ignore)]
        fn huge_axis_lengths_do_not_overflow(index in any::<isize>(), len in (usize::MAX / 2)..=usize::MAX) {
            for border in [BorderMode::<L8>::Reflect, BorderMode::Reflect101, BorderMode::Wrap] {
                prop_assert!(border.map_index(index, len).is_some_and(|m| m < len));
            }
        }
    }
}
