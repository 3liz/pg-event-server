//!
//! Postgres event dispatcher
//!
use futures::{stream, StreamExt};
use tokio::sync::mpsc;
use tokio_postgres::{error::DbError, tls::NoTls, AsyncMessage, Client};

use crate::{Config, Error, Notification, Result};

/// Listener for Postgres events
///
/// A pg event listener hold a connection to a database
/// and listen to event
///
/// A dispatcher is created with a `dispatch_id` that will
/// be associated will all sent messages
pub struct PgEventDispatcher {
    client: Client,
    config: Config,
    session_pid: i32,
    dispatch_id: i32,
}

#[derive(Debug, Clone)]
pub struct PgNotificationDispatch {
    notification: Notification,
    dispatch_id: i32,
}

impl PgNotificationDispatch {
    pub fn notification(&self) -> &Notification {
        &self.notification
    }
    pub fn dispatch_id(&self) -> i32 {
        self.dispatch_id
    }
    pub fn take_notification(self) -> Notification {
        self.notification
    }
}

impl PgEventDispatcher {
    /// Initialize a `PgEventDispatcher`
    pub async fn connect(
        config: Config, 
        tx: mpsc::Sender<PgNotificationDispatch>,
        dispatch_id: i32,
    ) -> Result<Self> {
        let (client, mut conn) = config.connect(NoTls).await?;

        // Create a stream from connection polling
        // see https://docs.rs/futures/latest/futures/stream/fn.poll_fn.html
        let mut stream = stream::poll_fn(move |cx| conn.poll_message(cx));

        // Send the connection in its own task
        // connection will close when the client will be dropped
        // or if an error occurs and the
        tokio::spawn(async move {
            log::debug!("Starting polling event");
            loop {
                if let Some(msg) = stream.next().await {
                    match msg {
                        Ok(msg) => match msg {
                            AsyncMessage::Notification(n) => {
                                if let Err(error) = tx.send(
                                        PgNotificationDispatch {
                                            notification: n,
                                            dispatch_id,
                                        }
                                    ).await {
                                    log::error!("{:?}", error);
                                    break;
                                }
                            }
                            AsyncMessage::Notice(dberr) => {
                                log_dberror_as_notice(&dberr);
                            }
                            _ => (),
                        },
                        Err(error) => {
                            log_postgres_error(&error);
                            break;
                        }
                    }
                } else {
                    log::debug!("Stream poll returned 'None'");
                }
            }
            log::debug!("Stop polling messages");
        });

        let session_pid = Self::get_session_pid(&client).await?;

        Ok(Self {
            client,
            config,
            session_pid,
            dispatch_id,
        })
    }

    /// Listen the specified channel
    pub async fn listen(&self, channel: &str) -> Result<()> {
        self.client
            .batch_execute(&format!("LISTEN {channel};"))
            .await
            .map_err(Error::from)
    }

    /// Unlisten the specified channel
    pub async fn unlisten(&self, channel: &str) -> Result<()> {
        self.client
            .batch_execute(&format!("UNLISTEN {channel};"))
            .await
            .map_err(Error::from)
    }

    /// Reconnect listener with its config
    pub async fn respawn(&mut self, tx: mpsc::Sender<PgNotificationDispatch>) -> Result<()> {
        let config = self.config.clone();
        Self::connect(config, tx, self.dispatch_id).await.map(|this| {
            *self = this;
        })
    }

    /// The configuration used for connection
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Return the pid session of the connection
    pub fn session_pid(&self) -> i32 {
        self.session_pid
    }

    /// The dispatch id of events sent by this dispatcher
    pub fn dispatch_id(&self) -> i32 {
        self.dispatch_id
    }

    async fn get_session_pid(client: &Client) -> Result<i32> {
        let row = client.query_one("SELECT pg_backend_pid();", &[]).await?;
        Ok(row.get(0))
    }

    /// Return true if the Listener is closed
    pub fn is_closed(&self) -> bool {
        self.client.is_closed()
    }
}

//
// Error logging
//
pub fn log_postgres_error(error: &Error) {
    if let Some(dberr) = error.as_db_error() {
        log_dberror_as_error(dberr);
    } else {
        log::error!("{:?}", error);
    }
}

fn log_dberror_as_notice(dberr: &DbError) {
    if log::log_enabled!(log::Level::Trace) {
        log::trace!("{}, {}", dberr.message(), dberr.detail().unwrap_or(""),);
    } else {
        log::debug!("{}", dberr.message());
    }
}

fn log_dberror_as_error(dberr: &DbError) {
    if log::log_enabled!(log::Level::Debug) {
        log::error!(
            "{}: {}",
            dberr.message(),
            dberr.detail().unwrap_or("(no detail)"),
        );
    } else {
        log::error!("{}", dberr.message());
    }
}
