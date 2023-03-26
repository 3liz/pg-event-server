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
use std::fs;
use std::path::Path;

use crate::errors::{Error, Result};

fn default_title() -> String {
    "Event Subscriber Service".into()
}

const fn default_worker_buffer_size() -> usize {
    10
}

const fn default_events_buffer_size() -> usize {
    1024
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
}

///
/// General Configuration
///
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Global server configuration
    pub server: Server,
    #[serde(default, rename(deserialize = "channel"))]
    pub channels: Vec<ChannelConfig>,

    /// worker buffer size
    #[serde(default = "default_worker_buffer_size")]
    pub worker_buffer_size: usize,

    /// events buffer size
    #[serde(default = "default_events_buffer_size")]
    pub events_buffer_size: usize
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
    pub allowed_events: Vec<String>,
    /// Connection string
    pub connection_string: String,
}

impl ChannelConfig {
    pub fn sanitize(&mut self) {
        self.id = self.id.trim_start_matches('/').into();
    }
}

///
/// Channel set config
///
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSetConfig {
    /// List of channel configuration in this
    /// channel set
    #[serde(default, rename(deserialize = "channel"))]
    pub channels: Vec<ChannelConfig>,
}

impl Config {
    /// Read configuration from `path`
    ///
    /// Will read channel configurations in a directory
    /// located in the same directory as the configuration file.
    pub fn read(path: &Path) -> Result<Self> {
        let mut conf: Self = toml::from_str(&fs::read_to_string(path)?)?;

        // Read all channel sets
        if let Some(stem) = path.file_stem().map(Path::new) {
            let confdir = path
                .parent()
                .unwrap_or(Path::new("./"))
                .join(stem.with_extension("d"));
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
                            conf.channels.append(&mut chanset.channels);
                        }
                        Err(err) => {
                            log::error!("Failed to read config file path: {err:?}");
                        }
                    }
                }
            }
        }
        Ok(conf)
    }

    /// Return the list of subscripitons
    pub fn subscriptions(&self) -> impl Iterator<Item=&str> {
        self.channels.iter().map(|c| c.id.as_ref())
    }
}

// Shortcut
pub fn read_config(path: &Path) -> Result<Config> {
    Config::read(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{confdir, setup};
    use std::env;

    #[test]
    fn load_configuration() {
        setup();
        let conf = Config::read(confdir!("config.toml")).unwrap();

        assert_eq!(conf.server.title, "Test server");
        assert_eq!(conf.channels.len(), 2);

        let chan0 = &conf.channels[0];
        assert_eq!(chan0.allowed_events, ["foo", "bar", "baz"]);
    }
}
