
# Refs

## SSE

* https://github.com/upbasedev/download-counter-example
* https://github.com/actix/examples/tree/master/server-sent-events
* https://dev.to/chaudharypraveen98/server-sent-events-in-rust-3lk0

## TLS
* https://www.reddit.com/r/rust/comments/upasc6/writing_a_tls_capable_http_proxy_in_rust_using/

## Postgres

Old example for handling Notify, may be should use [tokio-stream](https://docs.rs/tokio-stream/latest/tokio_stream/)

## Auth

* https://auth0.com/blog/build-an-api-in-rust-with-jwt-authentication-using-actix-web/

```
#![feature(poll_map)]
use futures::{stream, StreamExt};
use futures::{FutureExt, TryStreamExt};
use std::env;
use tokio::sync::mpsc;
use tokio_postgres::{connect, NoTls};

#[tokio::main]
async fn main() {
    let connection_parameters = env::var("DBURL").unwrap();
    let (client, mut conn) = connect(&connection_parameters, NoTls)
        .await
        .unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel();
    let stream = stream::poll_fn(move |cx| conn.poll_message(cx).map_err(|e| panic!(e)));
    let c = stream.forward(tx).map(|r| r.unwrap());
    tokio::spawn(c);
    println!("After spawn listener");

    client.batch_execute("LISTEN test_notifications;").await.unwrap();

    loop {
        let m = rx.recv().await;
        println!("GOT MESSAGE");
    }
}
```
