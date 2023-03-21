//!
//! SSE subscriber
//!
//! A channel may be open for any number of subscriptions.
//! Each subscription should be given a unique id.
//!
//!
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use serde::Deserialize;

use crate::Result;
use actix_web::{http::header::HeaderValue, rt, web, HttpRequest, Responder};
use actix_web_lab::sse;
use futures::{future, FutureExt};
use uuid::Uuid;

use log;

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

impl Broadcaster {
    /// Crate from configuration
    pub fn from_config(conf: &BroadcastConfig) -> Self {
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
                            subs.insert(id.into(), vec![chan]);
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
    pub async fn broadcast(&self, id: &str, event: &str, msg: &str) {
        let uuid = Uuid::new_v4().to_string();

        log::info!("BROADCAST({id},{event}) : {uuid}");

        if let Some(pool) = self.subs.borrow().get(id) {
            let res = future::join_all(pool.iter().map(|chan| {
                chan.sender
                    .send(sse::Data::new(msg).id(uuid.as_ref()).event(event))
                    .then(|result| {
                        let ident: &str = &chan.ident;
                        async move { (ident, result.is_ok()) }
                    })
            }))
            .await;
        }
    }

    /// Handler
    pub async fn handler(req: HttpRequest, bc: web::Data<Broadcaster>) -> Result<impl Responder> {
        let id: String = req.match_info().query("id").parse().unwrap();
        let ident: String = req
            .headers()
            .get("X-Identity")
            .unwrap_or(&HeaderValue::from_static(""))
            .to_str()
            .unwrap()
            .into();
        log::info!("REGISTER({id},{ident})");
        bc.new_channel(id, "".into()).await
    }

    /// Default buffer size value
    const fn default_buffer_size() -> usize {
        10
    }
}

/// Broadcast configuration
#[derive(Debug, Clone, Deserialize)]
pub struct BroadcastConfig {
    /// Size of buffer for pending responses     
    #[serde(default = "Broadcaster::default_buffer_size")]
    pub buffer_size: usize,
}

impl BroadcastConfig {}

impl Default for BroadcastConfig {
    fn default() -> Self {
        Self {
            buffer_size: Broadcaster::default_buffer_size(),
        }
    }
}
