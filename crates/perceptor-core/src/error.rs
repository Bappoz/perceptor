//! Tipo de erro da biblioteca.

use std::path::{Path, PathBuf};

/// Causa arbitrária preservada dentro de um [`Error`].
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Alias de `Result` com [`Error`] como tipo de erro padrão.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Erros produzidos pelas crates do Perceptor.
///
/// O enum é `#[non_exhaustive]`: novas variantes podem surgir em versões
/// menores, então um `match` externo precisa de um braço `_`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A operação exige um número específico de canais.
    #[error("esperado {expected} canal(is), recebido {actual}")]
    ChannelMismatch {
        /// Canais exigidos pela operação.
        expected: usize,
        /// Canais presentes na entrada.
        actual: usize,
    },

    /// O número de canais não tem representação no destino (ex.: formato de arquivo).
    #[error("número de canais não suportado: {channels}")]
    UnsupportedChannels {
        /// Canais presentes na entrada.
        channels: usize,
    },

    /// O comprimento do buffer não corresponde a um número inteiro de pixels.
    #[error("buffer com {len} valores não é múltiplo de {channels} canal(is)")]
    BufferLength {
        /// Número de subpixels no buffer.
        len: usize,
        /// Canais por pixel do formato pedido.
        channels: usize,
    },

    /// O buffer não está em memória contígua em ordem de linha.
    #[error("buffer de imagem não é contíguo em ordem de linha")]
    NonContiguous,

    /// Falha ao ler ou decodificar uma imagem.
    #[error("falha ao decodificar `{}`", .path.display())]
    Decode {
        /// Arquivo de origem.
        path: PathBuf,
        /// Erro original do decodificador ou do sistema de arquivos.
        #[source]
        source: BoxError,
    },

    /// Falha ao codificar ou gravar uma imagem.
    #[error("falha ao codificar `{}`", .path.display())]
    Encode {
        /// Arquivo de destino.
        path: PathBuf,
        /// Erro original do codificador ou do sistema de arquivos.
        #[source]
        source: BoxError,
    },
}

impl Error {
    /// Cria um [`Error::Decode`] preservando a causa.
    pub fn decode(path: impl AsRef<Path>, source: impl Into<BoxError>) -> Self {
        Self::Decode {
            path: path.as_ref().to_path_buf(),
            source: source.into(),
        }
    }

    /// Cria um [`Error::Encode`] preservando a causa.
    pub fn encode(path: impl AsRef<Path>, source: impl Into<BoxError>) -> Self {
        Self::Encode {
            path: path.as_ref().to_path_buf(),
            source: source.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn channel_mismatch_reports_both_counts() {
        let err = Error::ChannelMismatch {
            expected: 3,
            actual: 1,
        };
        assert_eq!(err.to_string(), "esperado 3 canal(is), recebido 1");
    }

    #[test]
    fn decode_keeps_path_and_source() {
        let source = std::io::Error::new(std::io::ErrorKind::NotFound, "sumiu");
        let err = Error::decode("a/b.png", source);
        assert_eq!(err.to_string(), "falha ao decodificar `a/b.png`");
        assert_eq!(err.source().unwrap().to_string(), "sumiu");
    }

    #[test]
    fn encode_keeps_path_and_source() {
        let err = Error::encode("out.png", "disco cheio");
        assert_eq!(err.to_string(), "falha ao codificar `out.png`");
        assert_eq!(err.source().unwrap().to_string(), "disco cheio");
    }

    #[test]
    fn unsupported_channels_reports_count() {
        let err = Error::UnsupportedChannels { channels: 2 };
        assert_eq!(err.to_string(), "número de canais não suportado: 2");
    }

    #[test]
    fn buffer_length_reports_len_and_channels() {
        let err = Error::BufferLength {
            len: 7,
            channels: 3,
        };
        assert_eq!(
            err.to_string(),
            "buffer com 7 valores não é múltiplo de 3 canal(is)"
        );
    }

    #[test]
    fn error_is_send_sync_static() {
        fn assert_bounds<T: Send + Sync + 'static>() {}
        assert_bounds::<Error>();
    }
}
