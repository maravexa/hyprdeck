use thiserror::Error;

#[derive(Debug, Error)]
pub enum FnordError {
    #[error("config error: {0}")]
    Config(String),

    #[allow(dead_code)]
    #[error("date error: {0}")]
    Date(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("parse error: {0}")]
    Parse(String),
}
