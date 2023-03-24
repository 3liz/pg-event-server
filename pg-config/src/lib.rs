//!
//! Connection string parsing with support for service file
//! and a subset of psql environment variables
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
//! * `PGCONNECT_TIMEOUT` - behaves the same as the `connect_timeout` connection parameter.
//!
//! ## See also
//!
//! * [Pg service file](https://www.postgresql.org/docs/current/libpq-pgservice.html)
//! * [Pg pass file](https://www.postgresql.org/docs/current/libpq-pgpass.html)
//!

use ini::Ini;
use std::path::{Path, PathBuf};
use std::str::FromStr;
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
    #[error("Missing service name in connection string")]
    MissingServiceName,
    #[error("Postgres config error")]
    PostgresConfig(#[from] tokio_postgres::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Load postgres connection configuration
///
/// The configuration will handle PG environment variable.
///
/// If the connection string start with `service=<service>`
/// the service will be searched in the file given by `PGSERVICEFILE`,
/// or in `~/.pg_service.conf` and `PGSYSCONFDIR/pg_service.conf`.
/// The remaining of the connection string is used directly for
/// initializing [`Config`]
///
/// If the connection string do no start with `service=` the connection
/// string is directly passed to [`Config`] along with parameters
/// defined for any service defined in `PGSERVICE`.
///
/// If the connection string is None the [`Config`] is initialized
/// from environment variables and/or service defined in `PGSERVICE`.
///
/// In all cases, parameters from the connection string take precedence.
///
pub fn load_pg_config(config: Option<&str>) -> Result<Config> {
    fn load_config(service: &str, cnxstr: &str) -> Result<Config> {
        if cnxstr.is_empty() {
            let mut config = Config::new();
            load_config_from_service(&mut config, service)?;
            load_config_from_env(&mut config)?;
            Ok(config)
        } else {
            let mut config = Config::from_str(cnxstr)?;
            load_config_from_service(&mut config, service)?;
            load_config_from_env(&mut config)?;
            Ok(config)
        }
    }

    if let Some(cnxstr) = config {
        let cnxstr = cnxstr.trim_start();
        if cnxstr.starts_with("service=") {
            // Get the service name
            // Assume the the tail is valid connection string
            if let Some((service, tail)) = cnxstr.split_once('=').map(|(_, tail)| {
                tail.split_once(|c: char| c.is_whitespace())
                    .unwrap_or((tail, ""))
            }) {
                load_config(service, tail.trim())
            } else {
                Err(Error::MissingServiceName)
            }
        } else if let Ok(service) = std::env::var("PGSERVICE") {
            // Service file defined
            // But overridable from connection string
            load_config(&service, cnxstr)
        } else {
            // No service defined
            let mut config = Config::from_str(cnxstr)?;
            load_config_from_env(&mut config)?;
            Ok(config)
        }
    } else if let Ok(service) = std::env::var("PGSERVICE") {
        load_config(&service, "")
    } else {
        // No service defined
        // Initialize from env vars.
        let mut config = Config::new();
        load_config_from_env(&mut config)?;
        Ok(config)
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
    static ENV: [(&str, &str); 7] = [
        ("PGHOST", "host"),
        ("PGPORT", "port"),
        ("PGDATABASE", "dbname"),
        ("PGUSER", "user"),
        ("PGOPTIONS", "options"),
        ("PGAPPNAME", "application_name"),
        ("PGCONNECT_TIMEOUT", "connect_timeout"),
    ];

    ENV.iter().try_for_each(|(varname, k)| {
        if let Ok(v) = std::env::var(varname) {
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
        // The following values may be set from
        // environment variables
        "user" => {
            if config.get_user().is_none() {
                config.user(v);
            }
        }
        "password" => {
            if config.get_password().is_none() {
                config.password(v);
            }
        }
        "dbname" => {
            if config.get_dbname().is_none() {
                config.dbname(v);
            }
        }
        "options" => {
            if config.get_options().is_none() {
                config.options(v);
            }
        }
        "host" | "hostaddr" => {
            if config.get_hosts().is_empty() {
                config.host(v);
            }
        }
        "port" => {
            if config.get_ports().is_empty() {
                config.port(v.parse().map_err(|_| Error::InvalidPort(v.into()))?);
            }
        }
        "connect_timeout" => {
            if config.get_connect_timeout().is_none() {
                config.connect_timeout(Duration::from_secs(
                    v.parse()
                        .map_err(|_| Error::InvalidConnectTimeout(v.into()))?,
                ));
            }
        }
        // The following are not set from environment variables
        // values are always overriden (i.e service configuration takes
        // precedence)
        "sslmode" => {
            config.ssl_mode(parse_ssl_mode(v)?);
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_postgres::config::Host;

    #[test]
    fn from_environment() {
        std::env::set_var("PGUSER", "foo");
        std::env::set_var("PGHOST", "foo.com");
        std::env::set_var("PGDATABASE", "foodb");
        std::env::set_var("PGPORT", "1234");

        let config = load_pg_config(None).unwrap();

        assert_eq!(config.get_user(), Some("foo"));
        assert_eq!(config.get_ports(), [1234]);
        assert_eq!(config.get_hosts(), [Host::Tcp("foo.com".into())]);
        assert_eq!(config.get_dbname(), Some("foodb"));
    }

    #[test]
    fn from_service_file() {
        std::env::set_var(
            "PGSYSCONFDIR",
            Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
                .join("fixtures")
                .to_str()
                .unwrap(),
        );

        let config = load_pg_config(Some("service=bar")).unwrap();

        assert_eq!(config.get_user(), Some("bar"));
        assert_eq!(config.get_ports(), [1234]);
        assert_eq!(config.get_hosts(), [Host::Tcp("bar.com".into())]);
        assert_eq!(config.get_dbname(), Some("bardb"));
    }

    #[test]
    fn service_override() {
        std::env::set_var(
            "PGSYSCONFDIR",
            Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
                .join("fixtures")
                .to_str()
                .unwrap(),
        );

        let config = load_pg_config(Some("service=bar user=baz")).unwrap();

        assert_eq!(config.get_user(), Some("baz"));
    }
}
