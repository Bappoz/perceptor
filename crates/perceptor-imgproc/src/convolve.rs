//! Correlação e convolução 2D com um kernel arbitrário: a **referência
//! escalar** dos filtros lineares.
//!
//! Cada pixel de saída é a soma ponderada da vizinhança do pixel de entrada,
//! canal a canal, com os pesos de um [`Kernel`] ancorado no centro:
//!
//! ```text
//! correlação:  saída(x, y) = Σ k(i, j) · entrada(x + i − cx, y + j − cy)
//! convolução:  saída(x, y) = Σ k(i, j) · entrada(x − i + cx, y − j + cy)
//! ```
//!
//! com `(cx, cy) = (largura/2, altura/2)`. A convolução é a correlação com o
//! kernel girado 180° ([`Kernel::flipped`]); para um kernel simétrico (blur,
//! Laplaciano) as duas coincidem, para um antissimétrico (Sobel) o sinal
//! troca. O `filter2D` do `OpenCV` é uma **correlação**: use [`correlate2d`]
//! para comparar resultados com ele.
//!
//! # Saída
//!
//! O formato de saída é escolhido por quem chama e precisa ter o mesmo número
//! de canais da entrada. A soma é feita em `f64` e convertida no fim:
//!
//! - saída inteira (`u8`, `u16`): arredonda para o mais próximo (metade para
//!   longe de zero) e **satura** em `[0, máx]`;
//! - saída `f32`: sem corte, com sinal — o que um gradiente precisa.
//!
//! Os valores **não são reescalados** entre profundidades: um `u8` `200`
//! entra na soma como `200.0`, qualquer que seja a saída.
//!
//! # Bordas
//!
//! Os vizinhos fora da imagem vêm do [`BorderMode`] passado. O kernel pode
//! ser maior que a imagem.
//!
//! # Custo
//!
//! `O(largura · altura · área do kernel)`, uma thread, sem aproveitar
//! separabilidade. É a versão simples e auditável contra a qual as otimizadas
//! são comparadas, não a que se usa em um caminho quente.

use perceptor_core::{BorderMode, Error, Image, ImageView, Pixel, Result};

use crate::convert::{check_channels, Sample};

/// Pesos de um filtro linear: grade `largura × altura` de lados ímpares,
/// ancorada no centro.
#[derive(Debug, Clone, PartialEq)]
pub struct Kernel {
    width: usize,
    height: usize,
    weights: Vec<f32>,
}

impl Kernel {
    /// Cria um kernel a partir dos pesos em ordem de linhas (*row-major*).
    ///
    /// # Errors
    /// [`Error::InvalidParameter`] se um dos lados for par ou zero, se
    /// `weights.len() != width · height` ou se algum peso não for finito.
    pub fn new(width: usize, height: usize, weights: Vec<f32>) -> Result<Self> {
        for (name, side) in [("width", width), ("height", height)] {
            if side.is_multiple_of(2) {
                return Err(Error::InvalidParameter {
                    name,
                    reason: "o lado do kernel precisa ser ímpar",
                });
            }
        }
        if width.checked_mul(height) != Some(weights.len()) {
            return Err(Error::InvalidParameter {
                name: "weights",
                reason: "a quantidade de pesos precisa ser largura × altura",
            });
        }
        if !weights.iter().all(|w| w.is_finite()) {
            return Err(Error::InvalidParameter {
                name: "weights",
                reason: "os pesos precisam ser finitos",
            });
        }
        Ok(Self {
            width,
            height,
            weights,
        })
    }

    /// Cria um kernel de um array de linhas, como se escreve no papel.
    ///
    /// ```
    /// use perceptor_imgproc::convolve::Kernel;
    ///
    /// let sobel_x = Kernel::from_rows([[-1.0, 0.0, 1.0], [-2.0, 0.0, 2.0], [-1.0, 0.0, 1.0]])?;
    /// assert_eq!((sobel_x.width(), sobel_x.height()), (3, 3));
    /// # Ok::<(), perceptor_core::Error>(())
    /// ```
    ///
    /// # Errors
    /// Os mesmos de [`Kernel::new`].
    pub fn from_rows<const W: usize, const H: usize>(rows: [[f32; W]; H]) -> Result<Self> {
        Self::new(W, H, rows.as_flattened().to_vec())
    }

    /// Número de colunas.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Número de linhas.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    /// Pesos em ordem de linhas.
    #[must_use]
    pub fn weights(&self) -> &[f32] {
        &self.weights
    }

    /// O kernel girado 180°: o peso de `(i, j)` vai para
    /// `(largura − 1 − i, altura − 1 − j)`.
    #[must_use]
    pub fn flipped(&self) -> Self {
        Self {
            width: self.width,
            height: self.height,
            // Em ordem de linhas, girar 180° é inverter a sequência.
            weights: self.weights.iter().rev().copied().collect(),
        }
    }
}

/// Correlação 2D: o peso `(i, j)` multiplica o vizinho deslocado de
/// `(i − cx, j − cy)`. É o que o `filter2D` do `OpenCV` calcula.
///
/// ```
/// use perceptor_core::{BorderMode, Image, L8, LF32};
/// use perceptor_imgproc::convolve::{correlate2d, Kernel};
///
/// // Diferença central horizontal: direita − esquerda.
/// let kernel = Kernel::from_rows([[-1.0, 0.0, 1.0]])?;
/// let image = Image::from_fn(3, 1, |x, _| L8::new([10, 30, 60][x]))?;
/// let dx: Image<LF32> = correlate2d(image.view(), &kernel, BorderMode::Replicate)?;
/// assert_eq!(dx.row(0), [LF32::new(20.0), LF32::new(50.0), LF32::new(30.0)]);
/// # Ok::<(), perceptor_core::Error>(())
/// ```
///
/// # Errors
/// - [`Error::ChannelMismatch`] se a saída tiver outro número de canais.
/// - [`Error::Allocation`] se a saída não puder ser alocada.
pub fn correlate2d<P, Q>(
    input: ImageView<'_, P>,
    kernel: &Kernel,
    border: BorderMode<P>,
) -> Result<Image<Q>>
where
    P: Pixel,
    Q: Pixel,
    P::Subpixel: Sample,
    Q::Subpixel: Sample,
{
    check_channels::<P, Q>()?;
    let (width, height) = (input.width(), input.height());
    let mut output = Image::<Q>::new(width, height)?;

    let (pad_x, pad_y) = (kernel.width / 2, kernel.height / 2);
    // Uma linha estendida por linha do kernel: `window[j][x + i]` é o vizinho
    // de `(x, y)` sob o peso `(i, j)`.
    let mut window = vec![Vec::with_capacity(width + 2 * pad_x); kernel.height];
    let mut acc = vec![0.0f64; P::CHANNELS];

    // `y`, `j` e `pad_y` cabem em `isize`: são limitados pelo tamanho de um buffer.
    let top = isize::try_from(pad_y).unwrap_or(isize::MAX);
    for (y, dst) in (0isize..).zip(output.rows_mut()) {
        for (j, row) in (0isize..).zip(&mut window) {
            input.padded_row(y + j - top, pad_x, border, row);
        }
        for (x, out) in dst.iter_mut().enumerate() {
            acc.fill(0.0);
            for (row, weights) in window.iter().zip(kernel.weights.chunks_exact(kernel.width)) {
                for (pixel, &weight) in row[x..x + kernel.width].iter().zip(weights) {
                    for (sum, value) in acc.iter_mut().zip(pixel.channels()) {
                        *sum += f64::from(weight) * value.to_f64();
                    }
                }
            }
            for (slot, &sum) in out.channels_mut().iter_mut().zip(&acc) {
                *slot = Q::Subpixel::from_f64(sum);
            }
        }
    }
    Ok(output)
}

/// Convolução 2D: a [`correlate2d`] com o kernel girado 180°.
///
/// Com um impulso na entrada, a saída é uma cópia do kernel (na correlação,
/// uma cópia girada); é isso que torna a convolução associativa, e é por isso
/// que dois filtros em sequência equivalem à convolução dos seus kernels.
///
/// # Errors
/// Os mesmos de [`correlate2d`].
pub fn convolve2d<P, Q>(
    input: ImageView<'_, P>,
    kernel: &Kernel,
    border: BorderMode<P>,
) -> Result<Image<Q>>
where
    P: Pixel,
    Q: Pixel,
    P::Subpixel: Sample,
    Q::Subpixel: Sample,
{
    correlate2d(input, &kernel.flipped(), border)
}
