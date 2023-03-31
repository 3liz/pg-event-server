//!
//! Postgres tls configuration
//!

#[cfg(not(any(feature = "with-openssl", feature = "with-rustls")))]
pub mod postgres_tls {
    use crate::Result;
    use serde::Deserialize;
    use tokio_postgres::tls::NoTls;

    #[derive(Default, Debug, Clone, Deserialize)]
    pub struct PgTlsConfig {
        // This makes deserialize happy
        dummy: Option<usize>,
    }

    pub type PgTlsConnect = NoTls;

    impl PgTlsConfig {
        pub fn make_tls_connect(&self) -> Result<PgTlsConnect> {
            Ok(PgTlsConnect {})
        }

        pub fn check(&self) -> Result<()> {
            Ok(())
        }
    }
}

#[cfg(feature = "with-openssl")]
pub mod postgres_tls {
    mod pgtls_openssl;
    pub use pgtls_openssl::*;
}

#[cfg(feature = "with-rustls")]
pub mod postgres_tls {
    mod pgtls_rustls;
    pub use pgtls_rustls::*;
}
