//!
//! Handle a pool of event listener
//!
use crate::{Config, Notification, PgEventDispatcher, Result};
use tokio::sync::mpsc;

/// Pool of PgEventListeners
pub struct PgEventDispatcherPool {
    pool: Vec<PgEventDispatcher>,
    tx: mpsc::Sender<Notification>,
    rx: mpsc::Receiver<Notification>,
}

impl Default for PgEventDispatcherPool {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel(1024);
        Self { pool: vec![], tx, rx }
    }
}

impl PgEventDispatcherPool {
    /// Find a listener based on the same config
    pub fn iter(&self) -> impl Iterator<Item = &PgEventDispatcher> {
        self.pool.iter()
    }

    /// Add a new dispatcher based on the given configuration
    /// Return the session id for that dispatcher
    pub async fn add(&mut self, config: &Config) -> Result<i32> {
        match self.pool.iter().find(|d| d.config() == config) {
            Some(dispatcher) => Ok(dispatcher.session_pid()),
            None => {
                let dispatcher =
                    PgEventDispatcher::connect(config.clone(), self.tx.clone()).await?;
                log::debug!(
                    "Pool: Created dispatcher  for session {}: {:#?}",
                    dispatcher.session_pid(),
                    dispatcher.config()
                );
                let session_pid = dispatcher.session_pid();
                self.pool.push(dispatcher);
                log::info!(
                    "Pool: Added pg_event dispatcher for session: {}",
                    session_pid
                );
                Ok(session_pid)
            }
        }
    }

    pub async fn recv(&mut self) -> Option<Notification> {
        self.rx.recv().await
    }
}
