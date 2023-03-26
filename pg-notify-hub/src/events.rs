//!
//! Handle postgres events
//!
//!
use pg_event_listener::{Config, Notification, PgEventDispatcher};
use tokio::sync::mpsc;

use crate::{config::ChannelConfig, Result};

/// Event broadcasted to
/// All workers
#[derive(Default, Debug, Clone)]
pub struct Event {
    id: String,
    event: String,
    session: i32,
    payload: String,
    channels: Vec<String>,
}

impl Event {
    /// Create new event from notification
    pub fn new(id: String, notification: Notification, channels: Vec<String>) -> Self {
        Self {
            id,
            session: notification.process_id(),
            event: notification.channel().into(),
            payload: notification.payload().into(),
            channels,
        }
    }
    /// Unique id for this event
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Iterator to channels to be notified
    pub fn channels(&self) -> impl Iterator<Item = &str> {
        self.channels.iter().map(|s| s.as_ref())
    }
    /// Return the postgres channel name
    pub fn event(&self) -> &str {
        &self.event
    }
    /// return the postgres session id
    pub fn session_pid(&self) -> i32 {
        self.session
    }
    /// Return the event payload
    pub fn payload(&self) -> &str {
        &self.payload
    }
}

/// Channel
pub struct Channel {
    /// Identifier for this channel
    id: String,
    /// Allowed events for this channel
    events: Vec<String>,
    /// The pid of the session for this
    /// channel
    session_pid: i32,
}

impl Channel {
    /// Create new [`Channel`]
    pub fn new(session_pid: i32, conf: ChannelConfig) -> Self {
        Self {
            id: conf.id,
            events: conf.allowed_events,
            session_pid,
        }
    }
    /// The identfier for this channel
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Return true if that Channel is listening
    /// for `event`
    pub fn is_listening_for(&self, event: &str) -> bool {
        self.events.iter().any(|e| *e == event)
    }
}

//
// Dispatcher
//

/// Channel pool
pub struct EventDispatch {
    pool: Vec<PgEventDispatcher>,
    channels: Vec<Channel>,
    rx: mpsc::Receiver<Notification>,
    tx: mpsc::Sender<Notification>,
}

impl EventDispatch {
    /// Initialize `EventDispatch`
    ///
    /// `buffer` is the channel buffer size:
    /// see [`tokio::sync::mpsc::channel`]
    pub fn new(buffer: usize) -> Self {
        let (tx, rx) = mpsc::channel(buffer);
        Self {
            pool: vec![],
            channels: vec![],
            rx,
            tx,
        }
    }

    /// Return an iterator over Channels
    pub fn iter_channels(&self) -> impl Iterator<Item=&Channel> {
        self.channels.iter()
    }

    /// Compare the configurations
    /// Return true if the host, user and database are the same
    fn use_same_connection(dispatcher: &PgEventDispatcher, config: &Config) -> bool {
        let this = dispatcher.config();
        this.get_hosts() == config.get_hosts()
            && this.get_dbname() == config.get_dbname()
            && this.get_user() == config.get_user()
    }

    /// Addd a new connection to the connection pool
    ///
    /// No new connection is created if a connection already exists which
    /// target the same host, user and database.
    pub async fn add_connection(&mut self, conf: &ChannelConfig) -> Result<i32> {
        async fn listen(dispatcher: &PgEventDispatcher, events: &[String]) -> Result<()> {
            for event in events.iter() {
                dispatcher.listen(event).await?;
            }
            Ok(())
        }

        // Created postgres configuration
        log::debug!("Loading configuration channel for {}", conf.id);
        let pgconfig = pg_config::load_pg_config(Some(&conf.connection_string))?;
        match self
            .pool
            .iter()
            .find(|d| Self::use_same_connection(d, &pgconfig))
        {
            Some(dispatcher) => {
                listen(dispatcher, &conf.allowed_events).await?;
                Ok(dispatcher.session_pid())
            }
            None => {
                let dispatcher = PgEventDispatcher::connect(pgconfig, self.tx.clone()).await?;
                log::debug!(
                    "Pool: Created dispatcher  for session {}: {:#?}",
                    dispatcher.session_pid(),
                    dispatcher.config()
                );
                listen(&dispatcher, &conf.allowed_events).await?;

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

    /// Add a channel from the configuration definition
    pub async fn add_channel(&mut self, conf: ChannelConfig) -> Result<()> {
        // Create postgres configuration
        // TODO Make sure that a channel with the same id does not already
        // exists.
        let session = self.add_connection(&conf).await?;
        self.channels.push(Channel::new(session, conf));
        Ok(())
    }

    /// Convenient method for adding channels from configuration
    /// collection
    pub async fn listen_from<T>(&mut self, channels: T) -> Result<()>
    where
        T: IntoIterator<Item = ChannelConfig>,
    {
        for conf in channels.into_iter() {
            self.add_channel(conf).await?;
        }
        Ok(())
    }

    /// Listen for events
    pub async fn dispatch<F>(&mut self, mut f: F)
    where
        F: FnMut(Event),
    {
        use uuid::Uuid;

        while let Some(notification) = self.rx.recv().await {
            let event = notification.channel();
            let session = notification.process_id();

            // Find all all channels for this event
            let channels: Vec<String> = self
                .channels
                .iter()
                .filter_map(|chan| chan.is_listening_for(event).then(|| chan.id().to_string()))
                .collect();

            if !channels.is_empty() {
                // Each event will have a unique identifier
                let id = Uuid::new_v4().to_string();
                log::info!("EVENT({session}) {event}: {id}");
                f(Event::new(id, notification, channels));
            } else {
                log::error!("Unprocessed event '{event}' for session '{session}'");
            }
        }
    }
}
