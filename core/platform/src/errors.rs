use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("other error: {0}")]
    Other(String),
}

pub type PlatformResult<T> = Result<T, PlatformError>;
