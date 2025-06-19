//!
//! Server configuration
//!
//! The server configuration is defined
//! in `TOML` file.
//!
//! ## The `[server]` section
//!
//! * `confdir` - Directory where to find resources
//! * `listen` - The socket addresses to listen to (as `"ip:port"` strings)
//!
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::{Error, Result};
use crate::postgres::tls::PgTlsConfig;

use config::{
    Config, Environment, FileFormat,
    builder::{ConfigBuilder, DefaultState},
};

fn default_title() -> String {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    format!("Pg event server v{VERSION}")
}

const fn default_worker_buffer_size() -> usize {
    1
}

const fn default_events_buffer_size() -> usize {
    1024
}

const fn default_reconnection_delay() -> u16 {
    60
}

const fn default_ssl_enabled() -> bool {
    false
}

///
/// Server global configuration
///
#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    /// The sockets addresses to listen to
    pub listen: String,

    /// Description of the server
    #[serde(default = "default_title")]
    pub title: String,

    /// Number of workers
    /// Optional: the default number of workers is half the number of Cpu
    /// (1 minimum)
    pub num_workers: Option<usize>,

    /// Enable ssl
    #[serde(default = "default_ssl_enabled")]
    pub ssl_enabled: bool,
    /// Server ssl key
    pub ssl_key_file: Option<PathBuf>,
    /// Server ssl cert
    pub ssl_cert_file: Option<PathBuf>,
}

// Handle SSL configuration
use crate::tls::{make_tls_config, TlsServerConfig};

impl Server {
    pub fn make_tls_config(&self) -> Result<Option<TlsServerConfig>> {
        if self.ssl_enabled {
            Some(make_tls_config(self)).transpose()
        } else {
            Ok(None)
        }
    }

    fn sanitize(&mut self, root: &Path) {
        if let Some(workers) = self.num_workers {
            if workers == 0 {
                self.num_workers = None;
            }
        }
        if let Some(ref ssl_key) = self.ssl_key_file {
            if !ssl_key.has_root() {
                self.ssl_key_file = Some(root.join(ssl_key));
            }
        }
        if let Some(ref ssl_cert) = self.ssl_cert_file {
            if !ssl_cert.has_root() {
                self.ssl_cert_file = Some(root.join(ssl_cert));
            }
        }
    }
}


///
/// Subscription channel configuration
///
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelConfig {
    /// Id to channel
    /// Used in subscription request
    pub id: String,
    /// List of events allowed to subscribe to
    /// If no events are defined then *all* events
    /// are allowed.
    #[serde(default)]
    pub allowed_events: Vec<String>,
    /// Connection string
    pub connection_string: Option<String>,
}

impl ChannelConfig {
    fn sanitize(&mut self) {
        self.id = self.id.trim_start_matches('/').into();
    }
}

///
/// Channel set config for drop in config files
///
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSetConfig {
    /// List of channel configuration in this
    /// channel set
    #[serde(default, rename(deserialize = "channel"))]
    pub channels: Vec<ChannelConfig>,
}


///
/// General Configuration
///
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// Global server configuration
    pub server: Server,
    #[serde(default, rename(deserialize = "channel"))]
    pub channels: Vec<ChannelConfig>,

    /// worker buffer size
    #[serde(default = "default_worker_buffer_size")]
    pub worker_buffer_size: usize,

    /// events buffer size
    #[serde(default = "default_events_buffer_size")]
    pub events_buffer_size: usize,

    /// Reconnection delay in seconds
    #[serde(default = "default_reconnection_delay")]
    pub reconnect_delay: u16,

    /// Postgres tls configuration
    pub postgres_tls: Option<PgTlsConfig>,
}


impl Settings {
    fn sanitize(&mut self, root: &Path) {
        self.channels.iter_mut().for_each(|c| c.sanitize());
        self.server.sanitize(root);
    }

    fn validate(self) -> Result<Self> {
        // Check for duplicated channel id
        let mut idents = HashSet::<&str>::new();
        self.channels.iter().try_for_each(|c| {
            if !idents.insert(&c.id) {
                Err(Error::Config(format!("Duplicated channel id: '{}'", c.id)))
            } else {
                Ok(())
            }
        })?;

        if let Some(conf) = &self.postgres_tls {
            conf.validate()?;
        }

        Ok(self)
    }
}

impl Settings {

    /// Configure so environment will be as CONF_KEY__VALUE
    fn builder() -> ConfigBuilder<DefaultState> {
        Config::builder().add_source(
            Environment::with_prefix("conf")
                .prefix_separator("_")
                .separator("__")
                .ignore_empty(true)
        )
    }

    fn build(settings: ConfigBuilder<DefaultState>, root: &Path) -> Result<Self> {
        settings
            .build()?
            .try_deserialize()
            .map_err(|err| Error::Config(format!("{}", err)))
            .and_then(|mut this: Self| {
                this.sanitize(root);
                this.validate()
            })
    }

    /// Load configuration from file
    ///
    /// Will read channel configurations in a directory
    /// located in the same directory as the configuration file.
    pub fn from_file(path: &Path) -> Result<Self> {
        let root = path.parent().unwrap_or(Path::new("./"));
        
        Self::build(
            Self::builder().add_source(config::File::from(path).format(FileFormat::Toml)),
            root,
        ).and_then(|this| this.read_channel_configs(path, root))
    }

    // Read "drop in" configurations
    fn read_channel_configs(mut self, path: &Path, root: &Path) -> Result<Self> {
        // Read all channel sets
        if let Some(stem) = path.file_stem().map(Path::new) {
            let confdir = root.join(stem.with_extension("d"));
            log::debug!("Looking for configuration in {}", confdir.display());
            if confdir.is_dir() {
                for entry in glob::glob(confdir.join("*.toml").to_str().ok_or(Error::Config(
                    format!("Invalid confdir {}", confdir.display()),
                ))?)
                .unwrap()
                {
                    match entry {
                        Ok(path) => {
                            log::info!("Loading channels configuration: {}", path.display());
                            let mut chanset: ChannelSetConfig =
                                toml::from_str(&fs::read_to_string(path)?)?;
                            self.channels.append(&mut chanset.channels);
                        }
                        Err(err) => {
                            log::error!("Failed to read config file path: {err:?}");
                        }
                    }
                }
            }
        }
        Ok(self)
    }
}

// Shortcut
pub fn read_config(path: &Path) -> Result<Settings> {
    Settings::from_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{confdir, setup};
    use std::env;

    #[test]
    fn load_configuration() {
        setup();

        unsafe { std::env::set_var("CONF_SERVER__TITLE", "My Title"); }

        let settings = Settings::from_file(confdir!("config.toml")).unwrap();

        assert_eq!(settings.server.title, "My Title");
        assert_eq!(settings.channels.len(), 2);

        let chan0 = &settings.channels[0];
        assert_eq!(chan0.allowed_events, ["foo", "bar", "baz"]);
    }
}
