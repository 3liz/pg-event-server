//!
//! Listen asynchronously to Postgres events.
//!

pub type Error = tokio_postgres::error::Error;
pub type Result<T, E = Error> = std::result::Result<T, E>;

use futures::{stream, StreamExt};
use futures::{FutureExt, TryStreamExt};
use tokio::sync::mpsc;
use tokio_postgres::{config::Config, tls::NoTls, AsyncMessage, Client, Notification};

/// Listener for Postgres events
///
/// A pg event listener hold a connection to a database
/// and listen to event
pub struct PgEventListener {
    client: Client,
    rx: mpsc::Receiver<Notification>,
    closed: bool,
    config: Config,
}

impl PgEventListener {
    /// Initialize a `PgEventListener`
    pub async fn connect(config: Config) -> Result<Self> {
        let (client, mut conn) = config.connect(NoTls).await?;

        let (tx, rx) = mpsc::channel(16);

        // Create a stream from connection polling
        // see https://docs.rs/futures/latest/futures/stream/fn.poll_fn.html
        let mut stream = stream::poll_fn(move |cx| conn.poll_message(cx));

        // Send the connection in its own task
        // connection will close when the client will be dropped
        // or if an error occurs
        tokio::spawn(async move {
            loop {
                if let Some(msg) = stream.next().await {
                    match msg {
                        Ok(msg) => match msg {
                            AsyncMessage::Notification(n) => {
                                tx.send(n).await;
                            }
                            AsyncMessage::Notice(dberr) => {
                                eprintln!("Notice: {}", dberr.message());
                            }
                            _ => (),
                        },
                        Err(error) => {
                            //XXX show detailed error
                            eprintln!("{error:?}");
                            break;
                        }
                    }
                } else {
                    eprintln!("*** Stream returned None");
                }
            }
        });

        Ok(Self {
            client,
            rx,
            closed: false,
            config,
        })
    }

    /// Reconnect listener with its config
    pub async fn respawn(&mut self) -> Result<()> {
        let config = self.config.clone();
        Self::connect(config).await.map(|this| {
            *self = this;
        })
    }

    /// The configuration used for connection
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Return the pid session of the connection
    pub async fn session_pid(&self) -> Result<i32> {
        let row = self.client.query_one("SELECT pg_backend_pid()", &vec![]).await?;
        Ok(row.get(0))
    }

    /// Return true if the Listener is closed
    pub fn is_closed(&self) -> bool {
        self.closed || self.client.is_closed()
    }

    /// Wait for the next message
    ///
    /// Return [`None`] if the listener is closed
    pub async fn recv(&mut self) -> Option<Notification> {
        if self.is_closed() {
            None
        } else if let Some(msg) = self.rx.recv().await {
            Some(msg)
        } else {
            // Sender has been closed at some point
            self.closed = true;
            None
        }
    }

    pub async fn listen(&self, channel: &str) -> Result<()> {
        self.client
            .batch_execute(&format!("LISTEN {channel};"))
            .await
            .map_err(Error::from)
    }

    pub async fn unlisten(&self, channel: &str) -> Result<()> {
        self.client
            .batch_execute(&format!("UNLISTEN {channel};"))
            .await
            .map_err(Error::from)
    }
}
