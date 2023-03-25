//!
//! SSE subscriber
//!
//! A channel may be open for any number of subscriptions.
//! Each subscription should be given a unique id.
//!
//!
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use crate::{config::BroadcastConfig, Result};
use actix_web::{http::header::HeaderValue, rt, web, HttpRequest, Responder};
use actix_web_lab::sse;
use futures::{future, FutureExt};

type ChanPool = Vec<Channel>;
type Subscriptions = RefCell<HashMap<String, ChanPool>>;

struct Channel {
    ident: String,
    sender: sse::Sender,
    timestamp: u64,
}

pub struct Broadcaster {
    buffer_size: usize,
    subs: Subscriptions,
}

// Handlers
impl Broadcaster {
    /// Subscrible handler
    pub async fn do_subscribe(req: HttpRequest, bc: web::Data<Rc<Self>>) -> Result<impl Responder> {
        let id: String = req.match_info().query("id").into();
        let ident: String = req
            .headers()
            .get("X-Identity")
            .unwrap_or(&HeaderValue::from_static("<anonymous>"))
            .to_str()
            .unwrap()
            .into();
        log::info!("REGISTER({id},{ident})");
        bc.new_channel(id, ident).await
    }
}

impl Broadcaster {
    /// Crate new Broadcaster
    pub fn new(conf: &BroadcastConfig) -> Self {
        Self {
            buffer_size: conf.buffer_size,
            subs: Subscriptions::default(),
        }
    }

    /// Create a new communication channel and register it
    pub async fn new_channel(&self, id: String, ident: String) -> Result<impl Responder> {
        let (tx, rx) = sse::channel(self.buffer_size);

        let chan = Channel {
            ident,
            sender: tx,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs(),
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
                    match subs.get_mut(&id) {
                        Some(pool) => pool.push(chan),
                        None => {
                            subs.insert(id, vec![chan]);
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
    pub async fn broadcast(&self, id: &str, event: &str, msg: &str, uuid: &str) {
        log::info!("BROADCAST({id},{event}) : {uuid}");

        if let Some(pool) = self.subs.borrow().get(id) {
            let res = future::join_all(pool.iter().map(|chan| {
                chan.sender
                    .send(sse::Data::new(msg).id(uuid).event(event))
                    .then(|result| {
                        let ident: &str = &chan.ident;
                        async move { (ident, result.is_ok()) }
                    })
            }))
            .await;
        }
    }
}
