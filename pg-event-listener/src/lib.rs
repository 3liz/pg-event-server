//!
//! Listen asynchronously to Postgres events.
//!

/// Generic errors wrapper
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Postgres error")]
    PgError(#[from] tokio_postgres::error::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>; 

use tokio_postgres::{ config::Config, Client, tls::NoTls };
use tokio::sync::mpsc;
use futures::{stream, StreamExt};
use futures::{FutureExt, TryStreamExt};

use postgres_service::tokio::load_connect_params;

/// Listener for Postgres events
///
/// A pg event listener hold a connection to a database 
/// and listen to event
pub struct PgEventListener  {
    client: Client,
}

impl PgEventListener {

    pub async connect(connect_str: &str) -> Result<Self> {
        let connect_str = connect_str.trim_start();
        let config = if connect_str.starts_with("service=") {
            let (_, service) =  connect
        } else {
            Config::from_str(conn)?;
        }

        let (client, mut conn) = tokio_postgres::connect(connect_str, NoTls).await?;

        let (tx, mut rx) = mpsc::unbounded_channel();

        let stream = stream::poll_fn(move |cx| conn.poll_message(cx));
        let c = stream.forward(tx).map(|r| {
            eprintln!("{r:?}")
            r
        );

        // Send the connection in its own task
        // connection will close when the client will be dropped
        // or if an error occurs
        tokio::spawn(c);

    }


    pub async fn listen(&self, channel: &str) -> Result<()> {
        client.batch_execute(&format!("LISTEN {channel};").await
    }

    pub async fn unlisten(&self, channel: &str) -> Result<()> {
        client.batch_execute(&format!("UNLISTEN {channel};").await
    }
}
