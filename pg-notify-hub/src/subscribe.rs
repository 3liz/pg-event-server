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
use std::time::{Duration, SystemTime};

use actix_web::{http::header::HeaderValue, rt, web, HttpRequest, Responder};
use actix_web_lab::sse;
use futures::{future, FutureExt};

use crate::{events::Event, Error, Result};

type ChanPool = Vec<Channel>;
type Subscriptions = RefCell<HashMap<String, ChanPool>>;

struct Channel {
    id: String,
    ident: String,
    sender: sse::Sender,
    timestamp: u64,
    realip_remote_addr: Option<String>,
    peer_addr: Option<String>,
}

pub struct Broadcaster {
    buffer_size: usize,
    subs: Subscriptions,
    allowed_subscriptions: HashSet<String>,
}

// Handlers
impl Broadcaster {
    /// Subscrible handler
    pub async fn do_subscribe(req: HttpRequest, bc: web::Data<Rc<Self>>) -> Result<impl Responder> {
        let id: String = req.match_info().query("id").into();

        if !bc.allowed_subscriptions.contains(&id) {
            return Err(Error::SubscriptionNotFound);
        }

        let ident: String = req
            .headers()
            .get("X-Identity")
            .unwrap_or(&HeaderValue::from_static("<anonymous>"))
            .to_str()
            .unwrap()
            .into();

        log::info!("SUBSCRIBE({id},{ident})");
        bc.new_channel(req, id, ident).await
    }
}

impl Broadcaster {
    /// Crate new Broadcaster
    pub fn new(buffer_size: usize, allowed_subscriptions: HashSet<String>) -> Self {
        Self {
            buffer_size,
            subs: Subscriptions::default(),
            allowed_subscriptions,
        }
    }

    /// Create a new communication channel and register it
    async fn new_channel(&self, req: HttpRequest, id: String, ident: String) -> Result<impl Responder> {
        let (tx, rx) = sse::channel(self.buffer_size);

        let connection_info = req.connection_info();
        let realip_remote_addr = connection_info.realip_remote_addr().map(String::from);
        let peer_addr = connection_info.peer_addr().map(String::from);

        let chan = Channel {
            id,
            ident,
            sender: tx,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs(),
            realip_remote_addr,
            peer_addr,
        };

        // Add channel to pool
        loop {
            // We cannot be sure that the
            // the collection is not actually borrowed
            // while broadcasting, prevent panicking
            // by testing borrowness and eventually
            // wait a little bit
            match self.subs.try_borrow_mut() {
                Ok(mut subs) => {
                    match subs.get_mut(&chan.id) {
                        Some(pool) => pool.push(chan),
                        None => {
                            subs.insert(chan.id.clone(), vec![chan]);
                        }
                    }
                    break;
                }
                Err(_) => {
                    rt::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        Ok(rx)
    }

    /// Broadcast event to all listener of the subscription `id`
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn broadcast(&self, event: &Event) {
        // We hold the borrow accross the await call
        // this may lead to potential problem because
        // we can do a mutable borrow during the execution
        // of the futures.
        //
        // This should be ok as long as in every other place where we
        // perform a mutable borrow we use the `try_borrow_mut()`
        // method to ensure availability.
        let subs = self.subs.borrow();
        let res = future::join_all(
            event
                .channels()
                .filter_map(|channel| subs.get(channel))
                .flat_map(|pool| pool.iter())
                .map(|chan| {
                    chan.sender
                        .send(
                            sse::Data::new(event.payload())
                                .id(event.id())
                                .event(event.event()),
                        )
                        .then(|result| {
                            let ident: &str = &chan.ident;
                            let ok = result.is_ok();
                            if !ok {
                                log::debug!("Connection closed for {ident}"); 
                            } else {
                                log::debug!(
                                    "SEND({},{}) {}: {}",
                                    chan.id,
                                    event.session_pid(),
                                    event.event(),
                                    event.id()
                                );
                            }
                            async move { (ident, ok) }
                        })
                }),
        )
        .await;
    }
}
