//! Reutilização de buffers de imagem entre frames.
//!
//! Em um pipeline de streaming cada frame teria o seu `Image::new`, e com
//! ele uma alocação e um `memset` do tamanho do frame. [`BufferPool`] guarda
//! os buffers devolvidos e os entrega de novo para pedidos com as mesmas
//! dimensões e o mesmo formato de pixel.
//!
//! A devolução é automática: [`PooledImage`] devolve o buffer no `Drop`.

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::image::Block;
use crate::{Image, Pixel, PixelFormat, Result};

/// Chave de agrupamento: buffers só são trocados entre pedidos idênticos.
type Key = (usize, usize, PixelFormat);

/// Contadores de um [`BufferPool`], para testes e diagnóstico.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PoolStats {
    /// Pedidos atendidos com uma alocação nova.
    pub allocations: usize,
    /// Pedidos atendidos com um buffer reaproveitado.
    pub reuses: usize,
    /// Buffers devolvidos que excederam o limite e foram liberados.
    pub discarded: usize,
    /// Bytes retidos agora, ociosos, no pool.
    pub idle_bytes: usize,
}

#[derive(Default)]
struct State {
    idle: HashMap<Key, Vec<Vec<Block>>>,
    stats: PoolStats,
}

struct Shared {
    /// Teto de bytes ociosos; buffers emprestados não contam.
    max_idle_bytes: usize,
    state: Mutex<State>,
}

/// Pool de buffers de imagem por `(largura, altura, formato)`.
///
/// É um handle barato de clonar (`Arc`) e pode ser usado de várias threads.
/// O limite de memória vale para os buffers *ociosos*: o que está emprestado
/// pertence a quem o segura, e um buffer devolvido além do limite é liberado
/// em vez de retido.
///
/// ```
/// use perceptor_core::{BufferPool, L8};
///
/// let pool = BufferPool::unbounded();
/// for _ in 0..10 {
///     let mut frame = pool.acquire::<L8>(640, 480)?;
///     frame.as_pixels_mut().fill(L8::new(7));
/// } // cada `frame` volta ao pool aqui
/// assert_eq!(pool.stats().allocations, 1);
/// # Ok::<(), perceptor_core::Error>(())
/// ```
#[derive(Clone)]
pub struct BufferPool {
    shared: Arc<Shared>,
}

impl BufferPool {
    /// Pool que retém até `max_idle_bytes` de buffers ociosos.
    ///
    /// Com `0` o pool nunca retém nada e se comporta como `Image::new`.
    #[must_use]
    pub fn new(max_idle_bytes: usize) -> Self {
        Self {
            shared: Arc::new(Shared {
                max_idle_bytes,
                state: Mutex::default(),
            }),
        }
    }

    /// Pool sem limite de retenção.
    ///
    /// Com tamanhos de frame que variam sem parar a memória ociosa só cresce;
    /// prefira [`BufferPool::new`] nesse caso.
    #[must_use]
    pub fn unbounded() -> Self {
        Self::new(usize::MAX)
    }

    /// Empresta uma imagem `width × height` de `P`.
    ///
    /// Um buffer reaproveitado **mantém o conteúdo do frame anterior**; use
    /// [`BufferPool::acquire_zeroed`] se o algoritmo lê antes de escrever.
    ///
    /// # Errors
    /// Os mesmos de [`Image::new`].
    pub fn acquire<P: Pixel>(&self, width: usize, height: usize) -> Result<PooledImage<P>> {
        let key = (width, height, P::FORMAT);
        let recycled = {
            let mut state = self.lock();
            let blocks = state.idle.get_mut(&key).and_then(Vec::pop);
            if let Some(blocks) = &blocks {
                state.stats.reuses += 1;
                state.stats.idle_bytes -= block_bytes(blocks);
            }
            blocks
        };

        let image = if let Some(blocks) = recycled {
            Image::from_blocks(width, height, blocks)
        } else {
            // Fora do lock: a alocação pode ser grande e lenta.
            let image = Image::new(width, height)?;
            self.lock().stats.allocations += 1;
            image
        };
        Ok(PooledImage {
            image: Some(image),
            pool: self.clone(),
        })
    }

    /// Como [`BufferPool::acquire`], mas com todos os pixels em zero.
    ///
    /// # Errors
    /// Os mesmos de [`Image::new`].
    pub fn acquire_zeroed<P: Pixel>(&self, width: usize, height: usize) -> Result<PooledImage<P>> {
        let mut image = self.acquire::<P>(width, height)?;
        image.as_pixels_mut().fill(P::zeroed());
        Ok(image)
    }

    /// Contadores atuais.
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        self.lock().stats
    }

    /// Libera todos os buffers ociosos. Os emprestados não são afetados.
    pub fn clear(&self) {
        let mut state = self.lock();
        state.idle.clear();
        state.stats.idle_bytes = 0;
    }

    fn release(&self, blocks: Vec<Block>, key: Key) {
        let bytes = block_bytes(&blocks);
        let mut state = self.lock();
        // `saturating_add`: com `unbounded()` o teto é `usize::MAX`.
        if state.stats.idle_bytes.saturating_add(bytes) > self.shared.max_idle_bytes {
            state.stats.discarded += 1;
            return;
        }
        state.stats.idle_bytes += bytes;
        state.idle.entry(key).or_default().push(blocks);
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // O estado é só contadores e listas: um pânico alheio não o deixa inconsistente.
        self.shared
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl std::fmt::Debug for BufferPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferPool")
            .field("max_idle_bytes", &self.shared.max_idle_bytes)
            .field("stats", &self.stats())
            .finish()
    }
}

fn block_bytes(blocks: &[Block]) -> usize {
    std::mem::size_of_val(blocks)
}

/// Imagem emprestada de um [`BufferPool`].
///
/// Faz `Deref` para [`Image<P>`] e devolve o buffer ao pool no `Drop`.
pub struct PooledImage<P: Pixel> {
    // `None` só depois de `into_inner`, para o `Drop` não devolver duas vezes.
    image: Option<Image<P>>,
    pool: BufferPool,
}

impl<P: Pixel> PooledImage<P> {
    /// Desliga a imagem do pool: o buffer passa a ser dono comum e não volta.
    #[must_use]
    #[allow(clippy::missing_panics_doc)] // `image` só é `None` depois de `into_inner`, que consome `self`
    pub fn into_inner(mut self) -> Image<P> {
        self.image.take().expect("imagem presente até o Drop")
    }
}

impl<P: Pixel> Deref for PooledImage<P> {
    type Target = Image<P>;

    fn deref(&self) -> &Image<P> {
        self.image.as_ref().expect("imagem presente até o Drop")
    }
}

impl<P: Pixel> DerefMut for PooledImage<P> {
    fn deref_mut(&mut self) -> &mut Image<P> {
        self.image.as_mut().expect("imagem presente até o Drop")
    }
}

impl<P: Pixel> Drop for PooledImage<P> {
    fn drop(&mut self) {
        if let Some(image) = self.image.take() {
            let key = (image.width(), image.height(), P::FORMAT);
            self.pool.release(image.into_blocks(), key);
        }
    }
}

impl<P: Pixel> std::fmt::Debug for PooledImage<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("PooledImage").field(&self.image).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, Rgb8, L8};

    #[test]
    fn same_size_frames_cost_one_allocation() {
        let pool = BufferPool::unbounded();
        for _ in 0..100 {
            let frame = pool.acquire::<Rgb8>(64, 48).unwrap();
            assert_eq!(frame.dimensions(), (64, 48));
        }
        let stats = pool.stats();
        assert_eq!(stats.allocations, 1);
        assert_eq!(stats.reuses, 99);
    }

    #[test]
    fn buffers_are_keyed_by_dimensions_and_format() {
        let pool = BufferPool::unbounded();
        drop(pool.acquire::<L8>(4, 4).unwrap());
        drop(pool.acquire::<L8>(8, 2).unwrap());
        drop(pool.acquire::<Rgb8>(4, 4).unwrap());
        assert_eq!(pool.stats().allocations, 3);
        assert_eq!(pool.stats().reuses, 0);
        drop(pool.acquire::<L8>(4, 4).unwrap());
        assert_eq!(pool.stats().reuses, 1);
    }

    #[cfg(feature = "rayon")]
    #[test]
    fn concurrent_frames_reuse_buffers() {
        use rayon::prelude::*;
        let pool = BufferPool::unbounded();
        (0..256).into_par_iter().for_each(|i| {
            let mut frame = pool.acquire::<L8>(32, 32).unwrap();
            frame
                .as_pixels_mut()
                .fill(L8::new(u8::try_from(i % 256).unwrap()));
        });
        let stats = pool.stats();
        assert_eq!(stats.allocations + stats.reuses, 256);
        // Cada thread segura no máximo um buffer por vez.
        assert!(stats.allocations <= rayon::current_num_threads());
    }

    #[test]
    fn idle_memory_is_bounded() {
        // 4×4 L8 ocupa um bloco de 64 bytes.
        let pool = BufferPool::new(64);
        let a = pool.acquire::<L8>(4, 4).unwrap();
        let b = pool.acquire::<L8>(4, 4).unwrap();
        drop(a);
        drop(b);
        let stats = pool.stats();
        assert_eq!(stats.idle_bytes, 64);
        assert_eq!(stats.discarded, 1);
        drop(pool.acquire::<L8>(4, 4).unwrap());
        assert_eq!(pool.stats().reuses, 1);
    }

    #[test]
    fn zero_limit_never_retains() {
        let pool = BufferPool::new(0);
        drop(pool.acquire::<L8>(4, 4).unwrap());
        drop(pool.acquire::<L8>(4, 4).unwrap());
        assert_eq!(pool.stats().allocations, 2);
        assert_eq!(pool.stats().idle_bytes, 0);
    }

    #[test]
    fn acquire_zeroed_clears_a_dirty_buffer() {
        let pool = BufferPool::unbounded();
        let mut frame = pool.acquire_zeroed::<L8>(4, 4).unwrap();
        frame.as_pixels_mut().fill(L8::new(255));
        drop(frame);
        let frame = pool.acquire_zeroed::<L8>(4, 4).unwrap();
        assert_eq!(pool.stats().reuses, 1);
        assert!(frame.as_pixels().iter().all(|&p| p == L8::new(0)));
    }

    #[test]
    fn into_inner_detaches_from_the_pool() {
        let pool = BufferPool::unbounded();
        let image = pool.acquire::<L8>(4, 4).unwrap().into_inner();
        drop(image);
        assert_eq!(pool.stats().idle_bytes, 0);
        drop(pool.acquire::<L8>(4, 4).unwrap());
        assert_eq!(pool.stats().allocations, 2);
    }

    #[test]
    fn invalid_dimensions_are_rejected() {
        let pool = BufferPool::unbounded();
        let err = pool.acquire::<L8>(0, 4).err().unwrap();
        assert!(matches!(err, Error::InvalidDimensions { .. }));
        assert_eq!(pool.stats(), PoolStats::default());
    }

    #[test]
    fn clear_releases_idle_buffers() {
        let pool = BufferPool::unbounded();
        drop(pool.acquire::<L8>(4, 4).unwrap());
        assert_eq!(pool.stats().idle_bytes, 64);
        pool.clear();
        assert_eq!(pool.stats().idle_bytes, 0);
    }
}
