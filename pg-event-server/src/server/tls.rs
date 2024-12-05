//!
//! Rust ls server configuration
//!
use crate::config::Server;
use crate::errors::{Error, Result};
use rustls_pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};

pub type TlsServerConfig = rustls::server::ServerConfig;

pub fn make_tls_config(config: &Server) -> Result<TlsServerConfig> {
    log::debug!("Configuring server TLS");

    let cert_path = config
        .ssl_cert_file
        .as_ref()
        .ok_or(Error::Config("Missing ssl cert file option".into()))?
        .as_path();
    let key_path = config
        .ssl_key_file
        .as_ref()
        .ok_or(Error::Config("Missing ssl key file option".into()))?
        .as_path();

    log::debug!("Loading SSL cert file at {cert_path:?}");
    let cert_chain = CertificateDer::pem_file_iter(cert_path)
        .map_err(|err| Error::ServerTls(format!("Failed to open client certificate: {err:?}")))?
        .map(|cert| {
            cert.map_err(|err| {
                Error::ServerTls(format!("Failed to read client certificate: {err:?}"))
            })
        })
        .collect::<Result<Vec<CertificateDer>, _>>()?;

    log::debug!("Loading SSL key file at {key_path:?}");
    let key = PrivateKeyDer::from_pem_file(key_path)
        .map_err(|err| Error::ServerTls(format!("Failed to read server key: {err:?}")))?;

    TlsServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .map_err(|err| Error::Config(format!("Failed to configure tls: {err:?}")))
}
