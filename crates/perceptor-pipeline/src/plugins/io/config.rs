//! Tipos de configuração do plugin de I/O.

/// Formato de arquivo usado ao salvar frames.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// PNG (sem perdas).
    #[default]
    Png,
    /// JPEG (com perdas).
    Jpeg,
}
