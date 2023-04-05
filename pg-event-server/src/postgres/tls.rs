//!
//! Postgres rustls connection
//!
use crate::{Error, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::{fs, io};

use tokio_postgres_rustls::MakeRustlsConnect;

#[derive(Default, Debug, Clone, Deserialize)]
pub struct PgTlsConfig {
    /// Server ca file
    /// The file should contain a sequence of PEM-formatted CA certificates.
    tls_ca_file: Option<PathBuf>,

    /// Client authentification key
    tls_client_auth_key: Option<PathBuf>,
    /// Client authentification cert
    tls_client_auth_cert: Option<PathBuf>,
}

pub type PgTlsConnect = MakeRustlsConnect;

impl PgTlsConfig {
    /// Load native ca certs
    fn load_native_certs(&self, roots: &mut rustls::RootCertStore) -> Result<()> {
        // https://docs.rs/rustls-native-certs/0.6.2/rustls_native_certs/
        for cert in rustls_native_certs::load_native_certs().map_err(|err| {
            Error::PostgresTlsError(format!("Failed to load platform certs: {err:?}"))
        })? {
            roots.add(&rustls::Certificate(cert.0)).map_err(|err| {
                Error::PostgresTlsError(format!("Adding native CA cert failed: {err:?}"))
            })?;
        }
        Ok(())
    }

    fn load_ca_file(&self, path: &Path, store: &mut rustls::RootCertStore) -> Result<()> {
        let mut f = io::BufReader::new(fs::File::open(path)?);
        rustls_pemfile::certs(&mut f)
            .and_then(|contents| {
                contents.into_iter().try_for_each(|data| {
                    store.add(&rustls::Certificate(data)).map_err(|err| {
                        io::Error::new(io::ErrorKind::InvalidData, format!("{err:?}"))
                    })
                })
            })
            .map_err(|err| {
                Error::PostgresTlsError(format!("Failed to load {path:?} as PEM file: {err:?}"))
            })
    }

    // Load certificat chain for client authentification
    fn load_client_auth_cert(&self, path: &Path) -> Result<Vec<rustls::Certificate>> {
        let mut f = io::BufReader::new(fs::File::open(path)?);
        rustls_pemfile::certs(&mut f)
            .map(|contents| contents.into_iter().map(rustls::Certificate).collect())
            .map_err(|err| {
                Error::PostgresTlsError(format!(
                    "Failed to load client certificat {path:?}: {err:?}"
                ))
            })
    }

    // Load private key for client authentification
    fn load_client_auth_key(&self, path: &Path) -> Result<rustls::PrivateKey> {
        let mut f = io::BufReader::new(fs::File::open(path)?);
        while let Some(item) = rustls_pemfile::read_one(&mut f).map_err(|err| {
            Error::PostgresTlsError(format!("Failed to read key file {path:?}: {err:?}"))
        })? {
            match item {
                rustls_pemfile::Item::RSAKey(key)
                | rustls_pemfile::Item::PKCS8Key(key)
                | rustls_pemfile::Item::ECKey(key) => return Ok(rustls::PrivateKey(key)),
                _ => continue,
            }
        }

        Err(Error::PostgresTlsError(format!("No key in {path:?}")))
    }

    pub fn make_tls_connect(&self) -> Result<PgTlsConnect> {
        let mut store = rustls::RootCertStore::empty();

        if let Some(cafile) = &self.tls_ca_file {
            self.load_ca_file(cafile.as_path(), &mut store)
        } else {
            self.load_native_certs(&mut store)
        }?;

        let builder = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(store);

        let builder = match (&self.tls_client_auth_cert, &self.tls_client_auth_key) {
            (Some(keyfile), Some(certfile)) => builder
                .with_single_cert(
                    self.load_client_auth_cert(keyfile.as_path())?,
                    self.load_client_auth_key(certfile.as_path())?,
                )
                .map_err(|err| {
                    Error::PostgresTlsError(format!("Failed to set client tls certs: {err:?}"))
                })?,
            (None, None) => builder.with_no_client_auth(),
            (_, _) => return Err(Error::Config("Invalid tls configuration".into())),
        };

        Ok(MakeRustlsConnect::new(builder))
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
    }
}
