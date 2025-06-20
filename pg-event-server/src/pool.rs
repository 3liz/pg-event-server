//!
//! Handle Postgres connection pool
//!
//! Maintains a pool of event dispatchers for each distinct database connection
//! configuration.
//!
//! This allows us to use the same number of connections independently
//! of the number of workers used.
//!
use futures::future;
use pg_event_listener::{Config, Notification, PgEventDispatcher};
use tokio::sync::mpsc;
use tokio_postgres::tls::NoTls;

use crate::postgres::tls::PgTlsConnect;
use crate::{config::ChannelConfig, Result};

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

pub struct Pool {
    pool: Vec<PgEventDispatcher>,
    tx: mpsc::Sender<PgNotificationDispatch>,
    tls: Option<PgTlsConnect>,
}

impl Pool {
    /// Create a new Pool that will forward notification to `tx`
    pub fn new(tx: mpsc::Sender<PgNotificationDispatch>, tls: Option<PgTlsConnect>) -> Self {
        Self {
            pool: vec![],
            tx,
            tls,
        }
    }

    /// Handle reconnection
    pub async fn reconnect(&mut self) {
        if !self.pool.iter().any(|d| d.is_closed()) {
            return;
        }

        let _ = future::join_all(self.pool.iter_mut().map(|dispatcher| async {
            if dispatcher.is_closed() {
                if let Err(err) = match &self.tls {
                    Some(tls) => dispatcher.respawn(tls.clone()).await,
                    None => dispatcher.respawn(NoTls).await,
                } {
                    let conf = dispatcher.config();
                    log::error!(
                        "Failed to reconnect to database {} on {:?}: {:?}",
                        conf.get_dbname().unwrap_or("<unknown>"),
                        conf.get_hosts(),
                        err
                    );
                } else {
                    let conf = dispatcher.config();
                    log::info!(
                        "Succeded to reconnect to database {} on {:?} (backend session: {})",
                        conf.get_dbname().unwrap_or("<unknown>"),
                        conf.get_hosts(),
                        dispatcher.session_pid(),
                    );
                }
            }
        }))
        .await;
    }

    /// Spaw a new dispatcher task
    async fn start_dispatcher(&self, config: Config) -> Result<PgEventDispatcher> {
        let (tx, mut rx) = mpsc::channel(1);

        // XXX The connect method is generic and return different type of
        // future.
        let dispatcher = match &self.tls {
            Some(tls) => PgEventDispatcher::connect(config, tx, tls.clone()).await?,
            None => PgEventDispatcher::connect(config, tx, NoTls).await?,
        };
        let dispatch_id = dispatcher.session_pid();
        let tx_fwd = self.tx.clone();
        // Wrap the event and forward it
        actix_web::rt::spawn(async move {
            while let Some(notification) = rx.recv().await {
                if let Err(error) = tx_fwd
                    .send(PgNotificationDispatch {
                        notification,
                        dispatch_id,
                    })
                    .await
                {
                    log::error!("{:?}", error);
                    break;
                }
            }
            log::trace!("Forward task terminated for dispatcher {dispatch_id}.")
        });
        Ok(dispatcher)
    }

    /// Addd a new connection to the connection pool
    ///
    /// No new connection is created if a connection already exists which
    /// target the same host, user and database.
    pub async fn add_connection(&mut self, conf: &ChannelConfig) -> Result<i32> {
        log::debug!("Adding connection to {:#?}", conf);
        async fn listen(dispatcher: &mut PgEventDispatcher, events: &[String]) -> Result<()> {
            for event in events.iter() {
                dispatcher.listen(event).await?;
            }
            Ok(())
        }

        let connection_string = conf.connection_string.as_deref();

        // Created postgres configuration
        log::debug!(
            "Loading configuration channel for {}: {}",
            conf.id,
            connection_string.unwrap_or("<no connection string>")
        );

        let pgconfig = pg_client_config::load_config(connection_string)?;
        match self
            .pool
            .iter_mut()
            .find(|d| Self::use_same_connection(d, &pgconfig))
        {
            Some(dispatcher) => {
                listen(dispatcher, &conf.allowed_events).await?;
                Ok(dispatcher.session_pid())
            }
            None => {
                let mut dispatcher = self.start_dispatcher(pgconfig).await?;
                listen(&mut dispatcher, &conf.allowed_events).await?;
                let session_pid = dispatcher.session_pid();
                self.pool.push(dispatcher);
                log::info!("Pool: Added pg_event dispatcher for session: {session_pid}");
                Ok(session_pid)
            }
        }
    }

    /// Compare the configurations
    /// Return true if the host, user and database are the same
    fn use_same_connection(dispatcher: &PgEventDispatcher, config: &Config) -> bool {
        let this = dispatcher.config();
        this.get_hosts() == config.get_hosts()
            && this.get_dbname() == config.get_dbname()
            && this.get_user() == config.get_user()
    }
}
