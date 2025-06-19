//!
//! Handle postgres events
//!
//! ## Operational mode
//!
//! 1. The postgres client receive a notification event.
//! 2. All channels attached to these clients are selected from
//!    their allowed event set.
//! 3. The event is forwarded to watchers along with the list
//!    of candidate channels.
//!
//!
use crate::{config::ChannelConfig, pool::PgNotificationDispatch, pool::Pool, Result};
use pg_event_listener::Notification;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::config::Settings;

pub type ChanId = usize;

// A simple readonly type for not allocating memory
// when we have only one element, which should be
// the vast majority of cases.
use crate::utils::Values;

type ChanIds = Values<ChanId>;

/// Event broadcasted to
/// All workers
#[derive(Default, Debug, Clone)]
pub struct Event {
    id: String,
    event: String,
    session: i32,
    payload: String,
    channels: ChanIds,
}

impl Event {
    /// Create new event from notification
    fn new(id: String, notification: Notification, channels: ChanIds) -> Self {
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
    /// Channels to be notified
    pub fn channels(&self) -> &[ChanId] {
        self.channels.as_slice()
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
    /// Allowed events for this channel
    events: Vec<String>,
    /// The event dispatch_id
    dispatch_id: i32,
}

impl Channel {
    /// Create new [`Channel`]
    pub fn new(dispatch_id: i32, conf: ChannelConfig) -> Self {
        Self {
            events: conf.allowed_events,
            dispatch_id,
        }
    }
    /// Return true if that Channel is listening
    /// for `event`
    pub fn is_listening_for(&self, dispatch_id: i32, event: &str) -> bool {
        self.dispatch_id == dispatch_id
            && (self.events.is_empty() || self.events.iter().any(|e| *e == event))
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
    pub async fn connect(settings: &Settings) -> Result<Self> {
        log::debug!("Initializing event dispatcher");
        let (tx, rx) = mpsc::channel(settings.events_buffer_size);
        let reconnect_delay = settings.reconnect_delay;
        let mut pool = Pool::new(
            tx,
            settings
                .postgres_tls
                .as_ref()
                .map(|tls| tls.make_tls_connect())
                .transpose()?,
        );

        let mut channels = Vec::<Channel>::with_capacity(settings.channels.len());
        for conf in settings.channels.iter() {
            // Create postgres configuration
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
            let ids = channels
                .iter()
                .enumerate()
                .filter_map(|(i, chan)| chan.is_listening_for(dispatch_id, event).then_some(i))
                .collect::<ChanIds>();

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
