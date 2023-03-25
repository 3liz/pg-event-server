//!
//! Listen asynchronously to Postgres events.
//!
mod dispatcher;
mod pool;

pub use dispatcher::PgEventDispatcher;
pub use pool::PgEventDispatcherPool;

pub type Error = tokio_postgres::error::Error;
pub type Result<T, E = Error> = std::result::Result<T, E>;

use tokio::sync::mpsc;

pub use tokio_postgres::{config::Config, Notification};

/// Listener for Postgres events
///
/// A pg event listener hold a connection to a database
/// and listen to event
pub struct PgEventListener {
    dispatcher: PgEventDispatcher,
    rx: mpsc::Receiver<Notification>,
}

impl PgEventListener {
    /// Initialize a `PgEventListener`
    pub async fn connect(config: Config) -> Result<Self> {
        let (tx, rx) = mpsc::channel(16);
        let dispatcher = PgEventDispatcher::connect(config, tx).await?;
        Ok(Self { dispatcher, rx })
    }

    /// Wait for the next message
    ///
    /// Return [`None`] if the listener is closed
    pub async fn recv(&mut self) -> Option<Notification> {
        if self.is_closed() {
            None
        } else {
            self.rx.recv().await
        }
    }

    /// Listen the specified channel
    #[inline]
    pub async fn listen(&self, channel: &str) -> Result<()> {
        self.dispatcher.listen(channel).await
    }

    /// Unlisten the specified channel
    #[inline]
    pub async fn unlisten(&self, channel: &str) -> Result<()> {
        self.dispatcher.unlisten(channel).await
    }

    /// The configuration used for connection
    #[inline]
    pub fn config(&self) -> &Config {
        self.dispatcher.config()
    }

    /// Return the pid session of the connection
    #[inline]
    pub fn session_pid(&self) -> i32 {
        self.dispatcher.session_pid()
    }

    /// Return true if the Listener is closed
    #[inline]
    pub fn is_closed(&self) -> bool {
        self.dispatcher.is_closed()
    }
}
