//! Corpo dos alvos de fuzzing de `perceptor-core`.
//!
//! Os alvos em `fuzz/fuzz_targets/` só repassam os bytes para cá. Manter a
//! lógica em uma biblioteca comum permite rodar o mesmo código em
//! `cargo test` (sem nightly) e deixa as asserções sob revisão normal.
//!
//! Cada função **entra em pânico se uma invariante for violada** (inclusive se
//! o resultado `Ok`/`Err` divergir do previsto por um oráculo independente) e
//! nunca por causa de uma entrada inválida — entrada inválida deve virar `Err`.
//!
//! O retorno é `true` quando a entrada chegou a construir um objeto válido e
//! conferiu todas as invariantes; `false` quando só exercitou os caminhos de
//! erro. Os testes usam isso para garantir que o harness não se limita a `Err`.
//!
//! ```text
//! just fuzz image_ctor 60
//! ```

use bytemuck::Pod;
use perceptor_core::{
    Bgr8, BorderMode, Image, ImagePixel, ImageView, ImageViewMut, La8, Pixel, Rect, Rgb8, RgbF32,
    Rgba8, L16, L8, LF32,
};

/// Leitor de bytes que devolve `0` quando a entrada acaba.
pub struct Input<'a> {
    data: &'a [u8],
}

impl<'a> Input<'a> {
    /// Começa a leitura em `data`.
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    fn byte(&mut self) -> u8 {
        match self.data.split_first() {
            Some((&byte, rest)) => {
                self.data = rest;
                byte
            }
            None => 0,
        }
    }

    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(std::array::from_fn(|_| self.byte()))
    }

    /// Inteiro em `0..=max`.
    fn small(&mut self, max: usize) -> usize {
        let raw = u16::from_le_bytes([self.byte(), self.byte()]);
        usize::from(raw) % (max + 1)
    }

    /// Inteiro que favorece valores de borda: `0`, `1`, potências e `usize::MAX`.
    fn edge(&mut self) -> usize {
        match self.byte() % 8 {
            0 => 0,
            1 => 1,
            2 => usize::MAX,
            3 => usize::MAX / 2,
            4 => usize::MAX / 2 + 1,
            5 => 1 << 32,
            _ => self.small(40),
        }
    }

    /// `isize` que favorece os extremos.
    fn signed(&mut self) -> isize {
        match self.byte() % 4 {
            0 => isize::MIN,
            1 => isize::MAX,
            // Reinterpreta os bits: o valor exato não importa, só a cobertura.
            _ => isize::try_from(self.u64().cast_signed()).unwrap_or(isize::MIN),
        }
    }

    fn rest(&mut self) -> &'a [u8] {
        std::mem::take(&mut self.data)
    }
}

/// `len` valores de `T` preenchidos com os bytes de `bytes` (e zeros depois).
fn samples<T: Pod>(bytes: &[u8], len: usize) -> Vec<T> {
    let mut out = vec![T::zeroed(); len];
    let raw: &mut [u8] = bytemuck::cast_slice_mut(&mut out);
    let n = raw.len().min(bytes.len());
    raw[..n].copy_from_slice(&bytes[..n]);
    out
}

/// Igualdade bit a bit; `==` em `f32` falha com `NaN`, que a entrada pode conter.
fn same<T: Pod>(a: &[T], b: &[T]) -> bool {
    bytemuck::cast_slice::<T, u8>(a) == bytemuck::cast_slice::<T, u8>(b)
}

fn pixel_from<P: Pixel>(input: &mut Input) -> P {
    let bytes = samples::<u8>(input.rest(), std::mem::size_of::<P>());
    bytemuck::pod_read_unaligned(&bytes)
}

macro_rules! per_format {
    ($sel:expr, $func:ident($($arg:expr),*)) => {
        match $sel % 8 {
            0 => $func::<L8>($($arg),*),
            1 => $func::<La8>($($arg),*),
            2 => $func::<Rgb8>($($arg),*),
            3 => $func::<Rgba8>($($arg),*),
            4 => $func::<Bgr8>($($arg),*),
            5 => $func::<L16>($($arg),*),
            6 => $func::<LF32>($($arg),*),
            _ => $func::<RgbF32>($($arg),*),
        }
    };
}

/// Construtores de `Image<P>` com dimensões e buffers arbitrários.
#[must_use]
pub fn image_ctor(data: &[u8]) -> bool {
    let mut input = Input::new(data);
    let sel = input.byte();
    per_format!(sel, image_ctor_for(&mut input))
}

fn image_ctor_for<P: Pixel>(input: &mut Input) -> bool {
    let (width, height) = (input.small(96), input.small(96));
    let mode = input.byte() % 4;
    let exact = input.byte().is_multiple_of(2);
    let channels = P::CHANNELS;
    let valid = width > 0 && height > 0;

    let (result, expected_ok, source): (_, bool, Option<Vec<P::Subpixel>>) = match mode {
        0 => (Image::<P>::new(width, height), valid, None),
        1 => (Image::filled(width, height, P::default()), valid, None),
        2 => {
            let len = if exact {
                width * height * channels
            } else {
                input.small(64)
            };
            let raw = samples::<P::Subpixel>(input.rest(), len);
            let ok = valid && len == width * height * channels;
            (Image::from_subpixels(width, height, &raw), ok, Some(raw))
        }
        _ => {
            let count = if exact {
                width * height
            } else {
                input.small(64)
            };
            let pixels = samples::<P>(input.rest(), count);
            let ok = valid && count == width * height;
            let raw = P::as_subpixels(&pixels).to_vec();
            (Image::from_vec(width, height, pixels), ok, Some(raw))
        }
    };

    assert_eq!(
        result.is_ok(),
        expected_ok,
        "construtor modo {mode}: {width}x{height} → {:?}",
        result.as_ref().map(Image::dimensions)
    );
    let Ok(image) = result else { return false };

    assert_eq!(image.dimensions(), (width, height));
    assert_eq!(image.as_pixels().len(), width * height);
    assert_eq!(image.as_subpixels().len(), width * height * channels);
    assert!(
        image
            .as_bytes()
            .as_ptr()
            .addr()
            .is_multiple_of(Image::<P>::ALIGNMENT),
        "buffer desalinhado"
    );
    if let Some(raw) = source {
        assert!(same(image.as_subpixels(), &raw), "conteúdo copiado difere");
    }
    let copy = image.clone();
    assert!(same(copy.as_pixels(), image.as_pixels()));
    let from_view = image.view().to_image().expect("cópia de visão válida");
    assert!(same(from_view.as_pixels(), image.as_pixels()));
    for y in 0..height.min(8) {
        assert_eq!(image.row(y).len(), width);
    }
    assert_eq!(image.view().pixels().count(), width * height);
    true
}

/// Resultado esperado de `ImageView::new`, calculado sem usar o código testado.
#[must_use]
pub fn view_new_should_succeed(
    width: usize,
    height: usize,
    stride: usize,
    len: usize,
    channels: usize,
) -> bool {
    if width == 0 || height == 0 {
        return false;
    }
    let Some(row) = width.checked_mul(channels) else {
        return false;
    };
    if stride < row {
        return false;
    }
    (height - 1)
        .checked_mul(stride)
        .and_then(|start| start.checked_add(row))
        .is_some_and(|required| required <= len)
}

/// Resultado esperado de `roi`: região não vazia e inteiramente dentro da visão.
#[must_use]
pub fn roi_should_succeed(region: Rect, width: usize, height: usize) -> bool {
    region.width > 0
        && region.height > 0
        && region
            .x
            .checked_add(region.width)
            .is_some_and(|r| r <= width)
        && region
            .y
            .checked_add(region.height)
            .is_some_and(|b| b <= height)
}

/// Geometria de visões, ROI, divisão em faixas e acesso com borda.
#[must_use]
pub fn view_geometry(data: &[u8]) -> bool {
    let mut input = Input::new(data);
    let sel = input.byte();
    per_format!(sel, view_geometry_for(&mut input))
}

#[allow(clippy::too_many_lines)] // uma sequência linear de checagens; dividir só espalharia o estado
fn view_geometry_for<P: Pixel>(input: &mut Input) -> bool {
    let channels = P::CHANNELS;
    // Metade das entradas usa geometria plausível, para chegar aos caminhos
    // válidos; a outra metade usa extremos, para exercitar os de erro.
    let (width, height, stride) = if input.byte().is_multiple_of(2) {
        let (width, height) = (input.small(16), input.small(16));
        (width, height, width * channels + input.small(6))
    } else {
        (input.edge(), input.edge(), input.edge())
    };
    let len = input.small(2048);
    let region = Rect::new(input.edge(), input.edge(), input.edge(), input.edge());
    let split_row = input.edge();
    let (bx, by) = (input.signed(), input.signed());
    let pad = input.small(64);
    let border_sel = input.byte();
    let fill: P = pixel_from(input);
    let buffer = samples::<P::Subpixel>(input.rest(), len);

    let created = ImageView::<P>::new(&buffer, width, height, stride);
    assert_eq!(
        created.is_ok(),
        view_new_should_succeed(width, height, stride, len, channels),
        "ImageView::new({width}, {height}, stride {stride}) com buffer de {len}"
    );
    let Ok(view) = created else {
        // Visão inválida: a mutável tem de concordar.
        let mut copy = buffer.clone();
        assert!(ImageViewMut::<P>::new(&mut copy, width, height, stride).is_err());
        return false;
    };

    assert_eq!(view.dimensions(), (width, height));
    assert_eq!(view.stride(), stride);
    assert_eq!(view.is_contiguous(), stride == width * channels);
    assert_eq!(view.as_pixels().is_some(), view.is_contiguous());
    for y in 0..height.min(32) {
        assert_eq!(view.row(y).len(), width);
    }
    assert!(view.get(width, 0).is_none() && view.get(0, height).is_none());
    assert!(view.get(width - 1, height - 1).is_some());

    // ROI.
    let roi = view.roi(region);
    assert_eq!(
        roi.is_ok(),
        roi_should_succeed(region, width, height),
        "roi {region:?}"
    );
    if let Ok(sub) = roi {
        assert_eq!(sub.dimensions(), (region.width, region.height));
        let corners = [(0, 0), (region.width - 1, region.height - 1)];
        for (cx, cy) in corners {
            let inner = sub.get(cx, cy).expect("canto da roi");
            let outer = view
                .get(region.x + cx, region.y + cy)
                .expect("canto no pai");
            assert!(
                same(&[*inner], &[*outer]),
                "roi desalinhada em ({cx}, {cy})"
            );
        }
    }

    // Borda.
    let border = match border_sel % 5 {
        0 => BorderMode::Constant(fill),
        1 => BorderMode::Replicate,
        2 => BorderMode::Reflect,
        3 => BorderMode::Reflect101,
        _ => BorderMode::Wrap,
    };
    let sampled = view.get_bordered(bx, by, border);
    if let BorderMode::Constant(value) = border {
        let outside = usize::try_from(bx).map_or(true, |x| x >= width)
            || usize::try_from(by).map_or(true, |y| y >= height);
        if outside {
            assert!(same(&[sampled], &[value]));
        }
    }
    let axis = input.edge();
    match border.map_index(bx, axis) {
        Some(i) => assert!(i < axis, "map_index({bx}, {axis}) = {i}"),
        None => assert!(axis == 0 || matches!(border, BorderMode::Constant(_))),
    }
    let mut out = Vec::new();
    view.padded_row(by, pad, border, &mut out);
    assert_eq!(out.len(), width + 2 * pad);

    // Visão mutável: divisão em faixas e preenchimento.
    let mut work = buffer.clone();
    let before = buffer.clone();
    let Ok(mut mutable) = ImageViewMut::<P>::new(&mut work, width, height, stride) else {
        panic!("visão mutável rejeitou geometria aceita pela imutável");
    };
    let mismatch_ok = mutable.copy_from(&view).is_ok();
    assert!(mismatch_ok, "copy_from de visão idêntica falhou");
    match mutable.split_at_row(split_row) {
        Ok((mut top, bottom)) => {
            assert!(split_row > 0 && split_row < height);
            assert_eq!(top.height(), split_row);
            assert_eq!(bottom.height(), height - split_row);
            top.fill(fill);
            for y in 0..top.height().min(16) {
                assert!(top.row(y).iter().all(|p| same(&[*p], &[fill])));
            }
            // A faixa de baixo não pode ter sido tocada.
            let untouched = ImageView::<P>::new(&before, width, height, stride)
                .expect("visão original")
                .roi(Rect::new(0, split_row, width, height - split_row))
                .expect("faixa inferior");
            for y in 0..bottom.height().min(16) {
                assert!(same(bottom.row(y), untouched.row(y)));
            }
        }
        Err(_) => assert!(split_row == 0 || split_row >= height),
    }
    true
}

/// Conversões de/para `ndarray::ArrayView3` com formas e strides arbitrários.
#[must_use]
pub fn interop_ndarray(data: &[u8]) -> bool {
    let mut input = Input::new(data);
    let sel = input.byte();
    per_format!(sel, interop_ndarray_for(&mut input))
}

fn interop_ndarray_for<P: Pixel>(input: &mut Input) -> bool {
    use ndarray::{ArrayView3, Axis, ShapeBuilder};

    let (h, w) = (input.small(6), input.small(6));
    let c = if input.byte().is_multiple_of(2) {
        P::CHANNELS
    } else {
        input.small(5)
    };
    let strides = (input.small(40), input.small(40), input.small(8));
    let invert = input.byte();
    let len = input.small(300);
    let buffer = samples::<P::Subpixel>(input.rest(), len);

    // Forma incompatível com o buffer: `from_shape` recusa, e isso é válido.
    let Ok(mut array) = ArrayView3::from_shape((h, w, c).strides(strides), &buffer) else {
        return false;
    };
    if invert % 2 == 1 {
        array.invert_axis(Axis(usize::from(invert / 2) % 3));
    }
    let fits = c == P::CHANNELS && w > 0 && h > 0;
    let value_at = |y: usize, x: usize, k: usize| array[[y, x, k]];

    let owned = Image::<P>::from_array3(array);
    assert_eq!(owned.is_ok(), fits, "Image::from_array3 {h}x{w}x{c}");
    if let Ok(image) = owned {
        for ((y, x, k), &expected) in array.indexed_iter() {
            let got = image.get(x, y).expect("pixel").channels()[k];
            assert!(same(&[got], &[expected]));
        }
        let back = image.to_array3();
        assert_eq!(back.dim(), (h, w, c));
        for (index, &v) in back.indexed_iter() {
            assert!(same(&[v], &[value_at(index.0, index.1, index.2)]));
        }
    }

    // A visão sem cópia pode recusar layouts não padrão, mas nunca aceitar o que não cabe.
    if let Ok(view) = ImageView::<P>::from_array3(array) {
        assert!(fits);
        assert_eq!(view.dimensions(), (w, h));
        for ((y, x, k), &v) in view.as_array3().indexed_iter() {
            assert!(same(&[v], &[value_at(y, x, k)]));
        }
    }
    true
}

/// Conversões de/para `image::ImageBuffer`.
#[must_use]
pub fn interop_image(data: &[u8]) -> bool {
    let mut input = Input::new(data);
    let sel = input.byte();
    match sel % 6 {
        0 => interop_image_for::<L8>(&mut input),
        1 => interop_image_for::<La8>(&mut input),
        2 => interop_image_for::<Rgb8>(&mut input),
        3 => interop_image_for::<Rgba8>(&mut input),
        4 => interop_image_for::<L16>(&mut input),
        _ => interop_image_for::<RgbF32>(&mut input),
    }
}

fn interop_image_for<P: ImagePixel>(input: &mut Input) -> bool {
    let (w, h) = (input.small(12), input.small(12));
    let exact = input.byte().is_multiple_of(2);
    let len = if exact {
        w * h * P::CHANNELS
    } else {
        input.small(64)
    };
    let raw = samples::<P::Subpixel>(input.rest(), len);

    let (w32, h32) = (
        u32::try_from(w).expect("até 12"),
        u32::try_from(h).expect("até 12"),
    );
    let Some(buffer) =
        image::ImageBuffer::<P::Image, Vec<P::Subpixel>>::from_raw(w32, h32, raw.clone())
    else {
        return false;
    };

    let owned = Image::<P>::from_image_buffer(&buffer);
    assert_eq!(owned.is_ok(), w > 0 && h > 0);
    if let Ok(image) = owned {
        assert!(same(image.as_subpixels(), &raw[..w * h * P::CHANNELS]));
        let back = image.to_image_buffer().expect("dimensões cabem em u32");
        assert!(same(back.as_raw(), &raw[..w * h * P::CHANNELS]));
    }
    assert_eq!(
        ImageView::<P>::from_image_buffer(&buffer).is_ok(),
        w > 0 && h > 0
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gerador xorshift: entradas reproduzíveis sem dependência.
    fn inputs(count: usize, seed: u64) -> impl Iterator<Item = Vec<u8>> {
        let mut state = seed | 1;
        (0..count).map(move |_| {
            let mut next = || {
                state ^= state >> 12;
                state ^= state << 25;
                state ^= state >> 27;
                state.wrapping_mul(0x2545_F491_4F6C_DD1D)
            };
            let len = usize::try_from(next() % 300).unwrap();
            (0..len).map(|_| next().to_le_bytes()[3]).collect()
        })
    }

    type Target = fn(&[u8]) -> bool;

    const TARGETS: [(&str, Target); 4] = [
        ("image_ctor", image_ctor),
        ("view_geometry", view_geometry),
        ("interop_ndarray", interop_ndarray),
        ("interop_image", interop_image),
    ];

    #[test]
    fn targets_accept_empty_and_degenerate_input() {
        for (_, target) in TARGETS {
            for data in [&[][..], &[0], &[255; 3], &[0; 64], &[255; 64], &[7; 1024]] {
                let _ = target(data);
            }
        }
    }

    #[test]
    fn targets_survive_pseudo_random_input_and_reach_the_deep_paths() {
        const COUNT: usize = 3000;
        for (name, target) in TARGETS {
            let reached = inputs(COUNT, 0x00C0_FFEE)
                .filter(|data| target(data))
                .count();
            // Se quase nada chega aos caminhos válidos o harness só testaria `Err`.
            assert!(
                reached * 20 >= COUNT,
                "{name}: só {reached}/{COUNT} entradas válidas"
            );
        }
    }

    #[test]
    fn view_oracle_matches_the_documented_rules() {
        assert!(view_new_should_succeed(4, 3, 4, 12, 1));
        assert!(view_new_should_succeed(4, 3, 6, 16, 1)); // última linha sem preenchimento
        assert!(!view_new_should_succeed(4, 3, 6, 15, 1));
        assert!(!view_new_should_succeed(0, 3, 4, 12, 1));
        assert!(!view_new_should_succeed(4, 3, 3, 12, 1)); // stride < linha
        assert!(!view_new_should_succeed(usize::MAX, 1, usize::MAX, 8, 3)); // overflow
        assert!(!view_new_should_succeed(2, usize::MAX, usize::MAX, 8, 1));
    }

    #[test]
    fn roi_oracle_rejects_empty_outside_and_overflowing_regions() {
        assert!(roi_should_succeed(Rect::new(1, 1, 2, 2), 4, 4));
        assert!(!roi_should_succeed(Rect::new(0, 0, 0, 2), 4, 4));
        assert!(!roi_should_succeed(Rect::new(3, 0, 2, 1), 4, 4));
        assert!(!roi_should_succeed(Rect::new(usize::MAX, 0, 2, 1), 4, 4));
    }

    #[test]
    fn input_reader_pads_with_zeros() {
        let mut input = Input::new(&[1]);
        assert_eq!(input.byte(), 1);
        assert_eq!(input.byte(), 0);
        assert_eq!(input.u64(), 0);
        assert_eq!(input.small(10), 0);
    }
}
