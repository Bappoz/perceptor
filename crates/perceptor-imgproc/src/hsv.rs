//! Conversões RGB ↔ HSV e RGB ↔ HSL.
//!
//! Os dois modelos descrevem a cor por **matiz** (o ângulo na roda de cores),
//! **saturação** e um terceiro eixo de claridade: o *valor* `V = max(R, G, B)`
//! no HSV, a *luminosidade* `L = (max + min) / 2` no HSL. Separar o matiz da
//! claridade é o que os torna úteis para segmentar por cor.
//!
//! # Convenções de faixa
//!
//! | Formato | Matiz | Saturação e V/L |
//! |---|---|---|
//! | [`Hsv8`], [`Hsl8`] | `0..180`: graus divididos por 2 | `0..=255` |
//! | [`HsvF32`], [`HslF32`] | `0.0..360.0`: graus | `0.0..=1.0` |
//!
//! O matiz de 8 bits usa meio grau por nível para caber em um byte; é a
//! convenção do `OpenCV`. Vermelho é `0`, verde `60` (120°) e azul `120` (240°).
//!
//! Cinzas não têm matiz: resultam em `H = 0, S = 0`. Na volta, o matiz é
//! ignorado quando `S = 0`, e um matiz fora da faixa dá a volta na roda
//! (`180` em `Hsv8` é vermelho de novo; `-120.0` em `HsvF32` é azul).
//!
//! # Precisão
//!
//! Em `f32` a ida e volta devolve o RGB original a menos de
//! [`F32_ROUND_TRIP_TOLERANCE`]. Em 8 bits a quantização perde informação —
//! 180 matizes × 256 saturações não cobrem as cores de um mesmo valor — e a
//! ida e volta erra até [`HSV8_ROUND_TRIP_TOLERANCE`] níveis por canal em
//! HSV e [`HSL8_ROUND_TRIP_TOLERANCE`] em HSL. Os dois limites são o máximo
//! medido sobre os 2²⁴ valores RGB.
//!
//! As funções em `f32` não saturam a entrada: valores fora da faixa nominal
//! produzem saídas fora da faixa.

use perceptor_core::{Hsl8, HslF32, Hsv8, HsvF32, Image, ImageView, Pixel, Result, Rgb8, RgbF32};

use crate::map_pixels;

/// Maior erro por canal, em níveis, de `Rgb8 → Hsv8 → Rgb8`.
pub const HSV8_ROUND_TRIP_TOLERANCE: u8 = 4;
/// Maior erro por canal, em níveis, de `Rgb8 → Hsl8 → Rgb8`.
pub const HSL8_ROUND_TRIP_TOLERANCE: u8 = 5;
/// Maior erro por canal de `RgbF32 → HSV/HSL → RgbF32` na faixa nominal.
pub const F32_ROUND_TRIP_TOLERANCE: f32 = 1e-5;

/// Formato RGB que tem correspondentes em HSV e HSL.
pub trait RgbColor: Pixel {
    /// O formato HSV com o mesmo tipo de subpixel.
    type Hsv: HsvColor<Rgb = Self>;
    /// O formato HSL com o mesmo tipo de subpixel.
    type Hsl: HslColor<Rgb = Self>;

    /// O pixel em HSV.
    fn to_hsv(self) -> Self::Hsv;
    /// O pixel em HSL.
    fn to_hsl(self) -> Self::Hsl;
}

/// Formato HSV.
pub trait HsvColor: Pixel {
    /// O formato RGB com o mesmo tipo de subpixel.
    type Rgb: Pixel;

    /// O pixel em RGB.
    fn to_rgb(self) -> Self::Rgb;
}

/// Formato HSL.
pub trait HslColor: Pixel {
    /// O formato RGB com o mesmo tipo de subpixel.
    type Rgb: Pixel;

    /// O pixel em RGB.
    fn to_rgb(self) -> Self::Rgb;
}

/// Converte RGB em HSV: `Rgb8 → Hsv8`, `RgbF32 → HsvF32`.
///
/// # Errors
/// [`Error::Allocation`](perceptor_core::Error::Allocation) se a saída não
/// puder ser alocada.
pub fn rgb_to_hsv<P: RgbColor>(input: ImageView<'_, P>) -> Result<Image<P::Hsv>> {
    map_pixels(input, RgbColor::to_hsv)
}

/// Converte HSV em RGB: `Hsv8 → Rgb8`, `HsvF32 → RgbF32`.
///
/// # Errors
/// [`Error::Allocation`](perceptor_core::Error::Allocation) se a saída não
/// puder ser alocada.
pub fn hsv_to_rgb<P: HsvColor>(input: ImageView<'_, P>) -> Result<Image<P::Rgb>> {
    map_pixels(input, HsvColor::to_rgb)
}

/// Converte RGB em HSL: `Rgb8 → Hsl8`, `RgbF32 → HslF32`.
///
/// # Errors
/// [`Error::Allocation`](perceptor_core::Error::Allocation) se a saída não
/// puder ser alocada.
pub fn rgb_to_hsl<P: RgbColor>(input: ImageView<'_, P>) -> Result<Image<P::Hsl>> {
    map_pixels(input, RgbColor::to_hsl)
}

/// Converte HSL em RGB: `Hsl8 → Rgb8`, `HslF32 → RgbF32`.
///
/// # Errors
/// [`Error::Allocation`](perceptor_core::Error::Allocation) se a saída não
/// puder ser alocada.
pub fn hsl_to_rgb<P: HslColor>(input: ImageView<'_, P>) -> Result<Image<P::Rgb>> {
    map_pixels(input, HslColor::to_rgb)
}

// ── Núcleo em f32 ──────────────────────────────────────────────────────────────

/// Matiz em graus, `[0, 360)`, e `(max, min)` dos canais.
///
/// O matiz é o setor da roda definido pelo maior canal, mais o desvio dado
/// pela diferença dos outros dois; sem diferença entre os canais (cinza) é 0.
fn hue(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    if delta <= 0.0 {
        return (0.0, max, min);
    }
    #[allow(clippy::float_cmp)] // `max` é cópia de um dos três, não resultado de conta
    let sector = if max == r {
        (g - b) / delta
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    let degrees = sector * 60.0;
    let degrees = if degrees < 0.0 {
        degrees + 360.0
    } else {
        degrees
    };
    // `-ε + 360` arredonda para 360 em f32.
    (if degrees >= 360.0 { 0.0 } else { degrees }, max, min)
}

fn rgb_to_hsv_f32(r: f32, g: f32, b: f32) -> [f32; 3] {
    let (hue, max, min) = hue(r, g, b);
    let saturation = if max > 0.0 { (max - min) / max } else { 0.0 };
    [hue, saturation, max]
}

fn rgb_to_hsl_f32(r: f32, g: f32, b: f32) -> [f32; 3] {
    let (hue, max, min) = hue(r, g, b);
    let lightness = f32::midpoint(max, min);
    let range = 1.0 - (2.0 * lightness - 1.0).abs();
    let saturation = if range > 0.0 {
        ((max - min) / range).min(1.0)
    } else {
        0.0
    };
    [hue, saturation, lightness]
}

/// RGB a partir do matiz, do croma (a diferença `max − min`) e do menor
/// canal. É o passo comum à volta de HSV e de HSL.
fn from_hue_chroma(hue: f32, chroma: f32, floor: f32) -> [f32; 3] {
    let sector = hue.rem_euclid(360.0) / 60.0;
    // O canal intermediário sobe e desce linearmente dentro de cada setor.
    let middle = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let rgb = match sector {
        s if s < 1.0 => [chroma, middle, 0.0],
        s if s < 2.0 => [middle, chroma, 0.0],
        s if s < 3.0 => [0.0, chroma, middle],
        s if s < 4.0 => [0.0, middle, chroma],
        s if s < 5.0 => [middle, 0.0, chroma],
        _ => [chroma, 0.0, middle],
    };
    rgb.map(|channel| channel + floor)
}

fn hsv_to_rgb_f32(hue: f32, saturation: f32, value: f32) -> [f32; 3] {
    let chroma = value * saturation;
    from_hue_chroma(hue, chroma, value - chroma)
}

fn hsl_to_rgb_f32(hue: f32, saturation: f32, lightness: f32) -> [f32; 3] {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    from_hue_chroma(hue, chroma, lightness - chroma / 2.0)
}

// ── f32 ────────────────────────────────────────────────────────────────────────

impl RgbColor for RgbF32 {
    type Hsv = HsvF32;
    type Hsl = HslF32;

    fn to_hsv(self) -> HsvF32 {
        HsvF32(rgb_to_hsv_f32(self.r(), self.g(), self.b()))
    }

    fn to_hsl(self) -> HslF32 {
        HslF32(rgb_to_hsl_f32(self.r(), self.g(), self.b()))
    }
}

impl HsvColor for HsvF32 {
    type Rgb = RgbF32;

    fn to_rgb(self) -> RgbF32 {
        RgbF32(hsv_to_rgb_f32(self.h(), self.s(), self.v()))
    }
}

impl HslColor for HslF32 {
    type Rgb = RgbF32;

    fn to_rgb(self) -> RgbF32 {
        RgbF32(hsl_to_rgb_f32(self.h(), self.s(), self.l()))
    }
}

// ── u8 ─────────────────────────────────────────────────────────────────────────

/// `[0, 1]` → `0..=255`, arredondando.
fn to_level(unit: f32) -> u8 {
    // `as` satura: valores fora de `[0, 255]` vão para o extremo.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let level = (unit * 255.0).round() as u8;
    level
}

fn unit(level: u8) -> f32 {
    f32::from(level) / 255.0
}

/// Graus → meio grau por nível, em `0..180`.
fn to_half_degrees(degrees: f32) -> u8 {
    // `degrees` está em `[0, 360)`: a metade arredondada vai de 0 a 180.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let half = (degrees / 2.0).round() as u8;
    half % 180
}

fn to_degrees(half: u8) -> f32 {
    f32::from(half) * 2.0
}

fn rgb8(rgb: [f32; 3]) -> Rgb8 {
    Rgb8::new(to_level(rgb[0]), to_level(rgb[1]), to_level(rgb[2]))
}

impl RgbColor for Rgb8 {
    type Hsv = Hsv8;
    type Hsl = Hsl8;

    fn to_hsv(self) -> Hsv8 {
        let [h, s, v] = rgb_to_hsv_f32(unit(self.r()), unit(self.g()), unit(self.b()));
        Hsv8::new(to_half_degrees(h), to_level(s), to_level(v))
    }

    fn to_hsl(self) -> Hsl8 {
        let [h, s, l] = rgb_to_hsl_f32(unit(self.r()), unit(self.g()), unit(self.b()));
        Hsl8::new(to_half_degrees(h), to_level(s), to_level(l))
    }
}

impl HsvColor for Hsv8 {
    type Rgb = Rgb8;

    fn to_rgb(self) -> Rgb8 {
        rgb8(hsv_to_rgb_f32(
            to_degrees(self.h()),
            unit(self.s()),
            unit(self.v()),
        ))
    }
}

impl HslColor for Hsl8 {
    type Rgb = Rgb8;

    fn to_rgb(self) -> Rgb8 {
        rgb8(hsl_to_rgb_f32(
            to_degrees(self.h()),
            unit(self.s()),
            unit(self.l()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worst(round_trip: impl Fn(Rgb8) -> Rgb8) -> (u8, Rgb8) {
        let mut worst = (0, Rgb8::default());
        for rgb in 0..1u32 << 24 {
            let [_, r, g, b] = rgb.to_be_bytes();
            let pixel = Rgb8::new(r, g, b);
            let back = round_trip(pixel);
            let error = r
                .abs_diff(back.r())
                .max(g.abs_diff(back.g()))
                .max(b.abs_diff(back.b()));
            if error > worst.0 {
                worst = (error, pixel);
            }
        }
        worst
    }

    /// Os limites documentados são o máximo real, não uma folga: se a
    /// aritmética mudar, este teste diz o novo valor.
    #[test]
    fn u8_round_trip_tolerances_are_the_measured_maximum() {
        let (hsv, at) = worst(|p| p.to_hsv().to_rgb());
        assert_eq!(hsv, HSV8_ROUND_TRIP_TOLERANCE, "HSV, pior caso em {at:?}");
        let (hsl, at) = worst(|p| p.to_hsl().to_rgb());
        assert_eq!(hsl, HSL8_ROUND_TRIP_TOLERANCE, "HSL, pior caso em {at:?}");
    }
}
