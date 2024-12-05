//!
//! Define errors
//!

/// Generic errors wrapper
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO Error")]
    IO(#[from] std::io::Error),
    #[error("Invalid configuration file")]
    ConfigFormat(#[from] toml::de::Error),
    #[error("SystemTime error")]
    SystemTime(#[from] std::time::SystemTimeError),
    #[error("Config error")]
    Config(String),
    #[error("Postgres connection error")]
    PostgresConnection(#[from] pg_client_config::Error),
    #[error("Postgres error")]
    Postgres(#[from] pg_event_listener::Error),
    #[error("Subscription do not exists")]
    SubscriptionNotFound,
    #[error("Postgres TLS error: {0}")]
    PostgresTls(String),
    #[error("Server TLS error: {0}")]
    ServerTls(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

use actix_web::http::{header::ContentType, StatusCode};
use actix_web::HttpResponse;

impl actix_web::ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .insert_header(ContentType::json())
            .finish()
    }
    fn status_code(&self) -> StatusCode {
        match *self {
            Error::SubscriptionNotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
