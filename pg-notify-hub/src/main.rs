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
mod events;
mod landingpage;
mod subscribe;

use subscribe::Broadcaster;

use errors::{Error, Result};
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;

use clap::{ArgAction, Parser};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Path to configuration file
    #[arg(long)]
    config: String,
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,
}

//
// Define 1 to N communication channel
//
// The dispatcher will run in the main thread.
// Each worker will run a listener that will
// send the event on each SSE subsriber channel.
//
use events::{Event, EventDispatch};
use tokio::sync::watch::{self, Receiver, Sender};
//
// Event dispatcher
//
async fn start_event_dispatcher(tx: Sender<Event>, conf: config::Config) -> Result<()> {
    let mut dispatcher = EventDispatch::new(conf.events_buffer_size);
    // Load channel configuration
    dispatcher.listen_from(conf.channels.into_iter()).await?;
    // Start dispatching
    actix_web::rt::spawn(async move {
        dispatcher
            .dispatch(|event| {
                if let Err(err) = tx.send(event) {
                    log::error!("Dispatch error: {err:?}");
                }
            })
            .await;
    });
    Ok(())
}
//
// Worker event listener
//
// Each worker will listen to the incoming events
// and will publish it to subscription channels.
//
fn start_event_listener(bc: Rc<Broadcaster>, mut rx: Receiver<Event>) {
    actix_web::rt::spawn(async move {
        while rx.changed().await.is_ok() {
            let ev = rx.borrow();
            bc.broadcast(&ev).await;
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

    let bind_address = conf.server.listen.clone();
    let worker_buffer_size = conf.worker_buffer_size;
    let allowed_subscriptions: HashSet<_> = conf.subscriptions().map(|s| s.into()).collect();

    eprintln!("Starting pg event server on: {}", bind_address);

    let (tx, rx) = watch::channel(Event::default());

    start_event_dispatcher(tx, conf).await?;

    HttpServer::new(move || {
        let broadcaster = Rc::new(Broadcaster::new(
            worker_buffer_size,
            allowed_subscriptions.clone(),
        ));

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
                    .route(
                        "/subscribe/{id:.*}",
                        web::get().to(Broadcaster::do_subscribe),
                    ),
            );

        app
    })
    .bind(&bind_address)?
    .run()
    .await
    .map_err(Error::from)
}

//
// Logger
//
fn init_logger(verbose: u8) {
    use env_logger::Env;

    let mut builder = env_logger::Builder::from_env(Env::default().default_filter_or("info"));

    match verbose {
        1 => builder.filter_level(LevelFilter::Debug),
        _ if verbose > 1 => builder.filter_level(LevelFilter::Trace),
        _ => &mut builder,
    }
    .init();
}

#[cfg(test)]
mod tests;
