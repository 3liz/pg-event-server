//!
//
//!
//! Postgres openssl connection
//!
use crate::{Error, Result};
use openssl::ssl;
use postgres_openssl::MakeTlsConnector;
use serde::Deserialize;
use std::path::PathBuf;

// XXX Figure how to add client auth key+cert

#[derive(Default, Debug, Clone, Deserialize)]
pub struct PgTlsConfig {
    /// Server ca file
    /// The file should contain a sequence of PEM-formatted CA certificates.
    tls_ca_file: Option<PathBuf>,
}

pub type PgTlsConnect = MakeTlsConnector;

impl PgTlsConfig {
    pub fn make_tls_connect(&self) -> Result<PgTlsConnect> {
        let mut builder = ssl::SslConnector::builder(ssl::SslMethod::tls())
            .map_err(|err| Error::PostgresTlsError(format!("{err:?}")))?;

        if let Some(cafile) = &self.tls_ca_file {
            builder.set_ca_file(cafile).map_err(|err| {
                Error::PostgresTlsError(format!("Cannot load CA certificates: {err:?}"))
            })?;
        }
        Ok(MakeTlsConnector::new(builder.build()))
    }

    pub fn check(&self) -> Result<()> {
        if let Some(cafile) = &self.tls_ca_file {
            if !cafile.as_path().is_file() {
                return Err(Error::Config(format!(
                    "CA file not found: {:?}",
                    self.tls_ca_file
                )));
            }
        }

        Ok(())
        /*
        match (&self.tls_client_auth_cert, &self.tls_client_auth_key) {
            (Some(keyfile), Some(certfile)) => {
                if !certfile.as_path().is_file() {
                    Err(Error::Config(format!(
                        "Client cert file not found: {certfile:?}",
                    )))
                } else if !certfile.as_path().is_file() {
                    Err(Error::Config(format!(
                        "Client key file not found: {keyfile:?}",
                    )))
                } else {
                    Ok(())
                }
            }
            (None, None) => Ok(()),
            (None, _) => Err(Error::Config("Missing client certfile.".into())),
            (_, None) => Err(Error::Config("Missing client keyfile.".into())),
        }
        */
    }

}
