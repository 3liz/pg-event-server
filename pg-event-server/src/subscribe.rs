//!
//! SSE subscriber
//!
//! A channel may be open for any number of subscriptions.
//! Each subscription should be given a unique id.
//!
//!
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
//use std::time::SystemTime;

use actix_web::{web, HttpRequest, Responder};
use actix_web_lab::sse;
use futures::future;
use uuid::Uuid;

use crate::{
    events::{ChanId, Event},
    Error, Result,
};

type Subscriptions = RefCell<HashMap<ChanId, Vec<Channel>>>;

struct Channel {
    id: ChanId,
    path: String,
    ident: Uuid,
    sender: sse::Sender,
    //timestamp: u64,
    realip_remote_addr: Option<String>,
    peer_addr: Option<String>,
    client_id: Option<String>,
}

impl Channel {
    fn client_id_str(&self) -> &str {
        self.client_id.as_deref().unwrap_or("<anonymous>")
    }
    fn realip_remote_addr(&self) -> Option<&str> {
        self.realip_remote_addr.as_deref()
    }

    fn peer_addr(&self) -> Option<&str> {
        self.peer_addr.as_deref()
    }
}

#[derive(Default)]
pub struct Broadcaster {
    buffer_size: usize,
    subs: Subscriptions,
    allowed_subscriptions: HashMap<String, ChanId>,
    pending_subscriptions: RefCell<Vec<Channel>>,
}

// Handlers
impl Broadcaster {
    /// Subscrible handler
    pub async fn do_subscribe(req: HttpRequest, bc: web::Data<Rc<Self>>) -> Result<impl Responder> {
        let channel = req.match_info().query("id");

        match bc.allowed_subscriptions.get(channel) {
            Some(id) => bc.new_channel(&req, channel, *id).await,
            None => Err(Error::SubscriptionNotFound),
        }
    }
}

impl Broadcaster {
    /// Crate new Broadcaster
    pub fn new(buffer_size: usize, channels: Vec<String>) -> Self {
        Self {
            buffer_size,
            allowed_subscriptions: channels
                .into_iter()
                .enumerate()
                .map(|(i, s)| (s, i))
                .collect(),
            ..Self::default()
        }
    }

    /// Create a new communication channel and register it
    async fn new_channel(
        &self,
        req: &HttpRequest,
        path: &str,
        id: ChanId,
    ) -> Result<impl Responder> {
        let client_id: Option<String> = req
            .headers()
            .get("X-Identity")
            .map(|s| s.to_str().unwrap().into());

        let connection_info = req.connection_info();
        let realip_remote_addr = connection_info.realip_remote_addr().map(String::from);
        let peer_addr = connection_info.peer_addr().map(String::from);

        let (tx, rx) = sse::channel(self.buffer_size);
        let chan = Channel {
            id,
            path: path.into(),
            ident: Uuid::new_v4(),
            sender: tx,
            //timestamp: SystemTime::now()
            //    .duration_since(SystemTime::UNIX_EPOCH)?
            //    .as_secs(),
            realip_remote_addr,
            peer_addr,
            client_id,
        };

        log::info!(
            "SUBSCRIBE({path},{}) <{}> (peer: '{}')",
            chan.client_id_str(),
            chan.realip_remote_addr().unwrap_or(""),
            chan.peer_addr().unwrap_or(""),
        );

        // Add channel to pool
        // We cannot be sure that the
        // the collection is not actually borrowed
        // while broadcasting, prevent panicking.
        match self.subs.try_borrow_mut() {
            Ok(mut subs) => match subs.get_mut(&chan.id) {
                Some(pool) => pool.push(chan),
                None => {
                    subs.insert(chan.id, vec![chan]);
                }
            },
            Err(_) => {
                // Add to pending suscriptions
                self.pending_subscriptions.borrow_mut().push(chan)
            }
        }

        Ok(rx)
    }

    /// Resolve pendings subscriptions that
    /// occured when adding new subscriptions
    fn resolve_pending_subscriptions(&self) {
        let mut pendings = self.pending_subscriptions.replace(Vec::new());
        if !pendings.is_empty() {
            log::info!("Resolving {} pending subscriptions", pendings.len());
            // Collect all pendings subcriptions
            let mut subs = self.subs.borrow_mut();
            pendings
                .drain(..)
                .for_each(|chan| match subs.get_mut(&chan.id) {
                    Some(pool) => pool.push(chan),
                    None => {
                        subs.insert(chan.id, vec![chan]);
                    }
                });
        }
    }

    /// Send event to subscribers
    async fn send_event(chan: &Channel, event: &Event) -> Option<Uuid> {
        let result = chan
            .sender
            .send(
                sse::Data::new(event.payload())
                    .id(event.id())
                    .event(event.event()),
            )
            .await;

        let ok = result.is_ok();
        if !ok {
            let ident = chan.ident;
            log::info!(
                "Connection closed for {ident} '{}' <{}> (peer: '{}')",
                chan.client_id_str(),
                chan.realip_remote_addr().unwrap_or(""),
                chan.peer_addr().unwrap_or(""),
            );
            Some(ident)
        } else {
            log::debug!(
                "SEND({},{}) {}: {}",
                chan.path,
                event.session_pid(),
                event.event(),
                event.id()
            );
            None
        }
    }

    #[allow(clippy::await_holding_refcell_ref)]
    async fn broadcast_event(&self, event: &Event) {
        // We hold the borrow accross the await call
        // this may lead to potential problem because
        // we can do a mutable borrow during the execution
        // of the futures.
        //
        // This should be ok as long as in every other place where we
        // perform a mutable borrow we use the `try_borrow_mut()`
        // method to ensure availability.
        let res = {
            let subs = self.subs.borrow();
            future::join_all(
                event
                    .channels()
                    .iter()
                    .filter_map(|channel| subs.get(channel))
                    .flat_map(|pool| pool.iter())
                    .map(|chan| Self::send_event(chan, event)),
            )
            .await
        }
        .into_iter()
        .flatten()
        .collect::<HashSet<_>>();

        if !res.is_empty() {
            // Clean up dead connections
            let mut subs = self.subs.borrow_mut();
            event.channels().iter().for_each(|channel| {
                if let Some(pool) = subs.get_mut(channel) {
                    pool.retain(|chan| {
                        let closed = res.contains(&chan.ident);
                        if closed {
                            log::debug!("Cleaning closed connection: {:?}", chan.ident);
                        }
                        !closed
                    })
                }
            })
        }
    }

    /// Broadcast event to all listener of the subscription `id`
    pub async fn broadcast(&self, event: &Event) {
        self.broadcast_event(event).await;

        // Resolve pendings subscriptions
        self.resolve_pending_subscriptions()
    }
}
