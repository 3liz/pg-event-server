//!
//! Handle postgres events
//!
//!
use crate::{config::ChannelConfig, pool::PgNotificationDispatch, pool::Pool, Result};
use pg_event_listener::Notification;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::config::Config;

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
    /// The event dispatch_id
    dispatch_id: i32,
}

impl Channel {
    /// Create new [`Channel`]
    pub fn new(dispatch_id: i32, conf: ChannelConfig) -> Self {
        Self {
            id: conf.id,
            events: conf.allowed_events,
            dispatch_id,
        }
    }
    /// The identfier for this channel
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Return true if that Channel is listening
    /// for `event`
    pub fn is_listening_for(&self, dispatch_id: i32, event: &str) -> bool {
        self.dispatch_id == dispatch_id && self.events.iter().any(|e| *e == event)
    }
}

//
// Dispatcher
//

/// Channel pool
pub struct EventDispatch {
    pool: Pool,
    channels: Vec<Channel>,
    rx: mpsc::Receiver<PgNotificationDispatch>,
    reconnect_delay: u16,
}

impl EventDispatch {
    /// Initialize `EventDispatch`
    ///
    /// `buffer` is the channel buffer size:
    /// see [`tokio::sync::mpsc::channel`]
    pub async fn connect(config: &Config) -> Result<Self> {
        let (tx, rx) = mpsc::channel(config.events_buffer_size);
        let reconnect_delay = config.reconnect_delay;
        let mut pool = Pool::new(tx);

        let mut channels = Vec::<Channel>::with_capacity(config.channels.len());
        for conf in config.channels.iter() {
            // Create postgres configuration
            // TODO Make sure that a channel with the same id does not already
            // exists.
            let dispatch = pool.add_connection(conf).await?;
            channels.push(Channel::new(dispatch, conf.clone()));
        }

        Ok(Self {
            pool,
            channels,
            rx,
            reconnect_delay,
        })
    }

    /// Pool handler in charge of reconnection
    fn start_pool_handler(mut pool: Pool, reconnect_delay: u16) {
        actix_web::rt::spawn(async move {
            loop {
                actix_web::rt::time::sleep(Duration::from_secs(reconnect_delay.into())).await;
                pool.reconnect().await;
            }
        });
    }

    /// Listen for event
    pub async fn dispatch<F>(self, mut f: F)
    where
        F: FnMut(Event),
    {
        let channels = self.channels;
        let mut rx = self.rx;

        Self::start_pool_handler(self.pool, self.reconnect_delay);

        use uuid::Uuid;

        while let Some(dispatch) = rx.recv().await {
            let event = dispatch.notification().channel();
            let remote_session = dispatch.notification().process_id();

            let dispatch_id = dispatch.dispatch_id();

            // Find all candidates channels for this event
            let ids: Vec<String> = channels
                .iter()
                .filter_map(|chan| {
                    chan.is_listening_for(dispatch_id, event)
                        .then(|| chan.id().to_string())
                })
                .collect();

            if !ids.is_empty() {
                // Each event will have a unique identifier
                let id = Uuid::new_v4().to_string();
                log::info!("EVENT({remote_session}) {event}: {id}");
                f(Event::new(id, dispatch.take_notification(), ids));
            } else {
                log::error!("Unprocessed event '{event}' for session '{remote_session}'");
            }
        }
    }
}
