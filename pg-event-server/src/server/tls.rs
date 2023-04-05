//!
//! Rust ls server configuration
//!
use crate::config::Server;
use crate::errors::{Error, Result};
use rustls::{Certificate, PrivateKey, ServerConfig as RustlsServerConfig};
use std::{fs, io};

pub type TlsServerConfig = RustlsServerConfig;

pub fn make_tls_config(config: &Server) -> Result<TlsServerConfig> {
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

    let cert_file = &mut io::BufReader::new(fs::File::open(cert_path)?);
    let key_file = &mut io::BufReader::new(fs::File::open(key_path)?);

    log::debug!("Loading SSL cert file at {cert_path:?}");
    let cert_chain = rustls_pemfile::certs(cert_file)
        .map(|contents| contents.into_iter().map(Certificate).collect())
        .map_err(|err| Error::Config(format!("Failed to read cert {cert_path:?} : {err:?}")))?;

    log::debug!("Loading SSL key file at {key_path:?}");
    let key = loop {
        match rustls_pemfile::read_one(key_file).map_err(|err| {
            Error::Config(format!("Failed to read tls key {key_path:?} : {err:?}"))
        })? {
            Some(rustls_pemfile::Item::RSAKey(key)) => break Some(key),
            Some(rustls_pemfile::Item::PKCS8Key(key)) => break Some(key),
            Some(rustls_pemfile::Item::ECKey(key)) => break Some(key),
            Some(_) => continue,
            None => break None,
        }
    }
    .map(PrivateKey);

    if key.is_none() {
        return Err(Error::Config(format!("No TLS key found for {key_path:?}")));
    }

    RustlsServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key.unwrap())
        .map_err(|err| Error::Config(format!("Failed to configure tls: {err:?}")))
}
