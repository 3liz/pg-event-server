//!
//! Define errors
//!

/// Generic errors wrapper
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO Error")]
    IOError(#[from] std::io::Error),
    #[error("Configuration Error")]
    ConfigError(#[from] toml::de::Error),
    #[error("SystemTime error")]
    SystemTimeError(#[from] std::time::SystemTimeError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl actix_web::ResponseError for Error {}
