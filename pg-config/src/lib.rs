//!
//! Connection string parsing with support for service file
//! and psql environment variables
//!
//! ## Environment variables
//!
//! * `PGSERVICE` - Name of the postgres service used for connection params.
//! * `PGSYSCONFDIR` - Location of the service files.
//! * `PGSERVICEFILE` - Name of the service file.
//! * `PGHOST` - behaves the same as the `host` connection parameter.
//! * `PGPORT` - behaves the same as the `port` connection parameter.
//! * `PGDATABASE` - behaves the same as the `database` connection parameter.
//! * `PGUSER` - behaves the same as the user connection parameter.
//! * `PGPASSFILE` - Specifies the name of the file used to store password.
//! * `PGOPTIONS` - behaves the same as the `options` parameter.
//! * `PGAPPNAME` - behaves the same as the `application_name` connection parameter.
//! * `PGSSLMODE` - behaves the same as the sslmode connection parameter.
//! * `PGCONNECT_TIMEOUT` - behaves the same as the `connect_timeout` connection parameter.
//! * `PGCHANNELBINDING` - behaves the same as the `channel_binding` connection parameter.
//!
//! ## See also
//!
//! * [Pg service file](https://www.postgresql.org/docs/current/libpq-pgservice.html)
//! * [Pg pass file](https://www.postgresql.org/docs/current/libpq-pgpass.html)
//!

use ini::Ini;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio_postgres::config::{ChannelBinding, Config, SslMode};

/// Error while parsing service file or
/// retrieving parameter from environment
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO Error")]
    IOError(#[from] std::io::Error),
    #[error("Service File Error")]
    PgServiceFileError(#[from] ini::Error),
    #[error("Service file not found")]
    PgServiceFileNotFound,
    #[error("Definition of service {0} not found")]
    PgServiceNotFound(String),
    #[error("Invalid ssl mode, expecting 'prefer', 'require' or 'disable': found '{0}'")]
    InvalidSslMode(String),
    #[error("Invalid port, expecting integer, found '{0}'")]
    InvalidPort(String),
    #[error("Invalid connect_timeout, expecting number of secs, found '{0}'")]
    InvalidConnectTimeout(String),
    #[error("Invalid keepalives, '1' or '0', found '{0}'")]
    InvalidKeepalives(String),
    #[error("Invalid keepalives, expecting number of secs, found '{0}'")]
    InvalidKeepalivesIdle(String),
    #[error("Invalid Channel Binding, expecting 'prefer', 'require' or 'disable': found '{0}'")]
    InvalidChannelBinding(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Load postgres connection configuration 
/// 
/// The configuration will handle PG environment variable.
/// 
/// If the connection string start with `service=<service>` 
/// the service will be searched in the file given by `PGSERVICEFILE`,
/// or in `~/.pg_service.conf` and `PGSYSCONFDIR/pg_service.conf`
///
pub fn load_pg_config(config: Option<&strP) -> Result<Config> {


    if let Some(config) = config {
        let connect_str = connect_str.trim_start();
            let config = if connect_str.starts_with("service=") {
                let (_, service) =  connect
            } else {
                Config::from_str(conn)?;
            }

        

}


/// Load connection parameters from config_file
fn load_config_from_service(config: &mut Config, service_name: &str) -> Result<()> {
    fn user_service_file() -> Option<PathBuf> {
        std::env::var("PGSERVICEFILE")
            .map(|path| Path::new(&path).into())
            .or_else(|_| {
                std::env::var("HOME").map(|path| Path::new(&path).join(".pg_service.conf"))
            })
            .ok()
    }

    fn sysconf_service_file() -> Option<PathBuf> {
        std::env::var("PGSYSCONFDIR")
            .map(|path| Path::new(&path).join("pg_service.conf"))
            .ok()
    }

    fn get_service_params(config: &mut Config, path: &Path, service_name: &str) -> Result<bool> {
        if path.exists() {
            Ini::load_from_file(path)
                .map_err(Error::from)
                .and_then(|ini| {
                    if let Some(params) = ini.section(Some(service_name)) {
                        params
                            .iter()
                            .try_for_each(|(k, v)| set_parameter(config, k, v))
                            .map(|_| true)
                    } else {
                        Ok(false)
                    }
                })
        } else {
            Err(Error::PgServiceFileNotFound)
        }
    }

    let found = match user_service_file() {
        Some(path) => get_service_params(config, &path, service_name)?,
        None => false,
    } || match sysconf_service_file() {
        Some(path) => get_service_params(config, &path, service_name)?,
        None => false,
    };

    if !found {
        Err(Error::PgServiceNotFound(service_name.into()))
    } else {
        Ok(())
    }
}

/// Load configuration from environment variables
fn load_config_from_env(config: &mut Config) -> Result<()> {
    static ENV: [&str; 10] = [
        "PGHOST",
        "PGPORT",
        "PGDATABASE",
        "PGUSER",
        "PGPASSFILE",
        "PGOPTIONS",
        "PGAPPNAME",
        "PGSSLMODE",
        "PGCONNECT_TIMEOUT",
        "PGCHANNELBINDING",
    ];

    ENV.iter().try_for_each(|k| {
        if let Ok(v) = std::env::var(k) {
            set_parameter(config, k, &v)
        } else {
            Ok(())
        }
    })
}

fn set_parameter(config: &mut Config, k: &str, v: &str) -> Result<()> {

    fn parse_ssl_mode(mode: &str) -> Result<SslMode> {
        match mode {
            "disable" => Ok(SslMode::Disable),
            "prefer" => Ok(SslMode::Prefer),
            "require" => Ok(SslMode::Require),
            _ => Err(Error::InvalidSslMode(mode.into())),
        }
    }

    fn parse_channel_binding(mode: &str) -> Result<ChannelBinding> {
        match mode {
            "disable" => Ok(ChannelBinding::Disable),
            "prefer" => Ok(ChannelBinding::Prefer),
            "require" => Ok(ChannelBinding::Require),
            _ => Err(Error::InvalidChannelBinding(mode.into())),
        }
    }

    match k {
        "user" => {
            config.user(v);
        }
        "password" => {
            config.password(v);
        }
        "dbname" => {
            config.dbname(v);
        }
        "options" => {
            config.options(v);
        }
        "application_name" => {
            config.application_name(v);
        }
        "sslmode" => {
            config.ssl_mode(parse_ssl_mode(v)?);
        }
        "host" | "hostaddr" => {
            config.host(v);
        }
        "port" => {
            config.port(v.parse().map_err(|_| Error::InvalidPort(v.into()))?);
        }
        "connect_timeout" => {
            config.connect_timeout(Duration::from_secs(
                v.parse()
                    .map_err(|_| Error::InvalidConnectTimeout(v.into()))?,
            ));
        }
        "keepalives" => {
            config.keepalives(match v {
                "1" => Ok(true),
                "0" => Ok(false),
                _ => Err(Error::InvalidKeepalives(v.into())),
            }?);
        }
        "keepalives_idle" => {
            config.keepalives_idle(Duration::from_secs(
                v.parse()
                    .map_err(|_| Error::InvalidKeepalivesIdle(v.into()))?,
            ));
        }
        "channel_binding" => {
            config.channel_binding(parse_channel_binding(v)?);
        }
        _ => (),
    }

    Ok(())
}
