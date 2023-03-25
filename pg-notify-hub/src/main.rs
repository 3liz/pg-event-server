//! Hub for Postgres notification to SSE broadcast
//!
//! # Dispatch model
//!
//! Event are dispatched from one source to all running actix-web workers.
//! Each worker is listening and pass the event to a  shared `Data` object
//! that hold the SSE opened connections.
//!
//! Since SSE connections are persistent objects there is no need to share connections
//! between workers and each thread look in its connection pool if there is a subscriber
//! to the passed events.
//!
use log::LevelFilter;

mod config;
mod errors;
mod landingpage;
mod subscribe;

use subscribe::Broadcaster;

use errors::{Error, Result};
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Path to configuration file
    #[arg(long)]
    config: String,
    #[arg(short, long)]
    verbose: bool,
}

/// Event broadcasted to
/// All workers
#[derive(Default, Debug, Clone)]
struct Event {
    id: String,
    event: String,
    msg: String,
}

//
// Define 1 to N communication channel
//
// The dispatcher will run in the main thread.
// Each worker will run a listener that will
// send the event on each SSE subsriber channel.
//
use tokio::sync::watch::{self, Receiver, Sender};
//
// Event dispatcher
//
fn start_event_dispatcher(tx: Sender<Event>) {
    use uuid::Uuid;
    actix_web::rt::spawn(async move {
        loop {
            actix_web::rt::time::sleep(Duration::from_secs(3)).await;
            let uuid = Uuid::new_v4().to_string();
            tx.send(Event {
                id: uuid,
                event: "foo".into(),
                msg: "Hello world".into(),
            });
        }
    });
}
//
// Event listener
//
fn start_event_listener(bc: Rc<Broadcaster>, mut rx: Receiver<Event>) {
    actix_web::rt::spawn(async move {
        log::info!("Event listener started");
        while rx.changed().await.is_ok() {
            let ev = rx.borrow();
            bc.broadcast("test", &ev.event, &ev.msg, &ev.id).await;
        }
    });
}

//
// Main
//
#[actix_web::main]
async fn main() -> Result<()> {
    use actix_web::{middleware::Logger, web, App, HttpServer};

    let args = Cli::parse();

    let conf = config::read_config(Path::new(&args.config))?;
    init_logger(args.verbose);

    eprintln!("Starting pg event server on: {}", conf.server.listen);

    let (tx, rx) = watch::channel(Event::default());

    start_event_dispatcher(tx);

    HttpServer::new(move || {
        let broadcaster = Rc::new(Broadcaster::new(&conf.broadcast));

        start_event_listener(broadcaster.clone(), rx.clone());

        let app = App::new()
            .wrap(Logger::default())
            .service(
                web::resource("/")
                    .name("landing_page")
                    .route(web::get().to(landingpage::handler)),
            )
            .service(
                web::scope("/events")
                    .app_data(web::Data::new(broadcaster))
                    .route("/subscribe/{id}", web::get().to(Broadcaster::do_subscribe)),
            );

        app
    })
    .bind(&conf.server.listen)?
    .run()
    .await
    .map_err(Error::from)
}

//
// Logger
//
fn init_logger(verbose: bool) {
    use env_logger::Env;

    let mut builder = env_logger::Builder::from_env(Env::default().default_filter_or("info"));

    if verbose {
        builder.filter_level(LevelFilter::Trace);
        eprintln!("Verbose mode on");
    }
    builder.init();
}

#[cfg(test)]
mod tests;
