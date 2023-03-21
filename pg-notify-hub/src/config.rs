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
use std::path::{Path, PathBuf};

use crate::errors::{Error, Result};
use crate::services::subscribe::BroadcastConfig;

fn default_title() -> String {
    "Event Subscriber Service".into()
}

/// Server global configuration
#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    /// The sockets addresses to listen to
    pub listen: String,

    /// Directory where to find resources
    pub confdir: PathBuf,

    /// Description of the server
    #[serde(default = "default_title")]
    pub title: String,
}

/// Configuration
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Global server configuration
    pub server: Server,
    #[serde(default)]
    pub broadcast: BroadcastConfig,
}

impl Config {
    /// Read configuration from `path`
    pub fn read(path: &Path) -> Result<Self> {
        toml::from_str(&fs::read_to_string(path)?).map_err(Error::from)
    }

    pub fn confdir(&self) -> &Path {
        &self.server.confdir
    }
}

// Shortcut
pub fn read_config(path: &Path) -> Result<Config> {
    Config::read(path)
}
