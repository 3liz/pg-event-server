//!
//! Postgres event dispatcher
//!
use futures::{stream, StreamExt};
use tokio::sync::mpsc;
use tokio_postgres::{error::DbError, tls::MakeTlsConnect, AsyncMessage, Client, Socket};

use crate::{Config, Error, Notification, Result};
use std::collections::HashSet;

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
    tx: mpsc::Sender<Notification>,
    events: HashSet<String>,
}

impl PgEventDispatcher {
    /// Initialize a `PgEventDispatcher`
    pub async fn connect<T>(config: Config, tx: mpsc::Sender<Notification>, tls: T) -> Result<Self>
    where
        T: MakeTlsConnect<Socket> + Clone + Sync + Send + 'static,
        T::Stream: Sync + Send,
    {
        let (client, mut conn) = config.connect(tls).await?;
        let sender = tx.clone();

        // Send the connection in its own task
        // connection will close when the client will be dropped
        // or if an error occurs and the
        tokio::spawn(async move {
            // Create a stream from connection polling
            // see https://docs.rs/futures/latest/futures/stream/fn.poll_fn.html
            let mut stream = stream::poll_fn(move |cx| conn.poll_message(cx));

            loop {
                if let Some(msg) = stream.next().await {
                    match msg {
                        Ok(msg) => match msg {
                            AsyncMessage::Notification(n) => {
                                if let Err(error) = sender.send(n).await {
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
            log::debug!("PG: Stopped polling postgres messages");
        });

        let session_pid = Self::get_session_pid(&client).await?;
        log::debug!("Created PG connection with pid {}", session_pid);

        Ok(Self {
            client,
            config,
            session_pid,
            tx,
            events: HashSet::new(),
        })
    }

    /// Listen the specified channel
    pub async fn listen(&mut self, channel: &str) -> Result<bool> {
        self.client
            .batch_execute(&format!("LISTEN {channel};"))
            .await
            .map(|_| self.events.insert(channel.into()))
            .map_err(Error::from)
    }

    /// Unlisten the specified channel
    pub async fn unlisten(&mut self, channel: &str) -> Result<bool> {
        self.client
            .batch_execute(&format!("UNLISTEN {channel};"))
            .await
            .map(|_| self.events.remove(channel))
            .map_err(Error::from)
    }

    /// Listen for multiple events
    pub async fn batch_listen<T>(&mut self, events: T) -> Result<()>
    where
        T: IntoIterator<Item = String>,
    {
        let mut query: String = "".into();
        // Build a batch query
        self.events.extend(events.into_iter().inspect(|s| {
            query += "LISTEN ";
            query += s;
            query += ";";
        }));
        if !query.is_empty() {
            self.client.batch_execute(&query).await.map_err(Error::from)
        } else {
            Ok(())
        }
    }

    /// Reconnect listener with its config
    pub async fn respawn<T>(&mut self, tls: T) -> Result<()>
    where
        T: MakeTlsConnect<Socket> + Clone + Sync + Send + 'static,
        T::Stream: Sync + Send,
    {
        let config = self.config.clone();
        let events = self.events.drain().collect::<Vec<_>>();
        *self = Self::connect(config, self.tx.clone(), tls).await?;
        self.batch_listen(events).await
    }

    /// The configuration used for connection
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Return the pid session of the connection
    pub fn session_pid(&self) -> i32 {
        self.session_pid
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
        log::error!("PG: {:?}", error);
    }
}

fn log_dberror_as_notice(dberr: &DbError) {
    if log::log_enabled!(log::Level::Trace) {
        log::trace!("PG: {}, {}", dberr.message(), dberr.detail().unwrap_or(""),);
    } else {
        log::debug!("PG: {}", dberr.message());
    }
}

fn log_dberror_as_error(dberr: &DbError) {
    if log::log_enabled!(log::Level::Debug) {
        log::error!(
            "PG: {}: {}",
            dberr.message(),
            dberr.detail().unwrap_or("(no detail)"),
        );
    } else {
        log::error!("PG: {}", dberr.message());
    }
}
