use thiserror::Error;

/// Erros legíveis em português. São enviados ao front como texto.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Caminho não encontrado: {0}")]
    NotFound(String),
    #[error("O caminho aponta para uma pasta. Escolha um arquivo.")]
    IsDirectory,
    #[error("O arquivo está vazio.")]
    Empty,
    #[error("Arquivo grande demais ({0} MB). O limite é de 512 MB.")]
    TooLarge(u64),
    #[error("Não foi possível ler o arquivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("Nenhum arquivo aberto. Abra um arquivo primeiro.")]
    NoSession,
    #[error("Função não encontrada: {0}")]
    FunctionNotFound(String),
    #[error("Endereço inválido: {0}")]
    BadAddress(String),
    #[error("Escopo ou tipo de código desconhecido: {0}")]
    BadArgument(String),
}
