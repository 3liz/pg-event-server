//!
//! Postgres rustls connection
//!
use crate::{Error, Result};
use rustls_pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use serde::Deserialize;
use std::path::{Path, PathBuf};
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
    fn load_native_certs(roots: &mut rustls::RootCertStore) -> Result<()> {
        log::debug!("Loading native certs");
        // https://docs.rs/rustls-native-certs/latest/rustls_native_certs/
        let results = rustls_native_certs::load_native_certs();

        results
            .errors
            .iter()
            .for_each(|err| log::error!("Failed to load certificat {:?}", err));

        for cert in results.certs {
            if let Err(err) = roots.add(cert) {
                log::error!("Adding native CA cert failed: {:?}", err);
            }
        }
        Ok(())
    }

    fn load_ca_file(path: &Path, roots: &mut rustls::RootCertStore) -> Result<()> {
        log::debug!("Using custom postgres certificat {:?}", path);
        CertificateDer::pem_file_iter(path)
            .map_err(|err| Error::PostgresTls(format!("Failed to open CA certificate: {err:?}")))?
            .try_for_each(|cert| match cert {
                Ok(cert) => roots.add(cert).map_err(|err| {
                    Error::PostgresTls(format!("Failed to load {path:?} as PEM file: {err:?}"))
                }),
                Err(err) => Err(Error::PostgresTls(format!("Certificat error {err:?}"))),
            })
    }

    // Load certificat chain for client authentification
    fn load_client_auth_cert(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
        CertificateDer::pem_file_iter(path)
            .map_err(|err| {
                Error::PostgresTls(format!("Failed to open client certificate: {err:?}"))
            })?
            .map(|cert| {
                cert.map_err(|err| {
                    Error::PostgresTls(format!("Failed to read client certificate: {err:?}"))
                })
            })
            .collect()
    }

    // Load private key for client authentification
    fn load_client_auth_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
        PrivateKeyDer::from_pem_file(path)
            .map_err(|err| Error::PostgresTls(format!("Failed to read client key: {err:?}")))
    }

    pub fn make_tls_connect(&self) -> Result<PgTlsConnect> {
        log::debug!("Configuring TLS for postgres clients");

        let mut store = rustls::RootCertStore::empty();

        if let Some(cafile) = &self.tls_ca_file {
            Self::load_ca_file(cafile, &mut store)
        } else {
            Self::load_native_certs(&mut store)
        }?;

        let builder = rustls::ClientConfig::builder().with_root_certificates(store);

        let builder = match (&self.tls_client_auth_cert, &self.tls_client_auth_key) {
            (Some(keyfile), Some(certfile)) => {
                let cert = Self::load_client_auth_cert(certfile)?;
                let key = Self::load_client_auth_key(keyfile)?;
                builder.with_client_auth_cert(cert, key).map_err(|err| {
                    Error::PostgresTls(format!("Failed to set client tls certs: {err:?}"))
                })?
            }
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
