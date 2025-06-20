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
mod pool;
mod postgres;
mod subscribe;
mod tls;
mod utils;

use subscribe::Broadcaster;

use errors::{Error, Result};
use std::path::Path;
use std::rc::Rc;

use clap::{ArgAction, Parser};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Path to configuration file
    #[arg(long)]
    conf: String,
    /// Increase verbosity
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,
    /// Check configuration only
    #[arg(long)]
    check: bool,
}

//
// Define M to N communication channel with
// tokio::async::watch
//
// The dispatcher will run in the main thread.
// Each worker will run a listener that will
// send the event on each SSE subsriber channel.
//
use crate::events::{Event, EventDispatch};
use tokio::sync::watch::{self, Receiver, Sender};
//
// Event dispatcher
//
async fn start_event_dispatcher(tx: Sender<Event>, settings: &config::Settings) -> Result<()> {
    let dispatcher = EventDispatch::connect(settings).await?;
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
    use actix_web::{
        middleware::{DefaultHeaders, Logger},
        web, App, HttpServer,
    };

    let args = Cli::parse();

    init_logger(args.verbose);

    let settings = config::read_config(Path::new(&args.conf))?;

    if args.check {
        println!("Configuration looks ok.");
        return Ok(());
    }

    let title = settings.server.title.clone();
    let bind_address = settings.server.listen.clone();
    let worker_buffer_size = settings.worker_buffer_size;
    let channels = settings
        .channels
        .iter()
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    let num_workers = settings
        .server
        .num_workers
        .unwrap_or_else(num_cpus::get_physical);

    let tls_config = settings.server.make_tls_config()?;

    let (tx, rx) = watch::channel(Event::default());

    log::info!("Starting Event dispatcher");
    start_event_dispatcher(tx, &settings).await?;

    let server = HttpServer::new(move || {
        let broadcaster = Rc::new(Broadcaster::new(worker_buffer_size, channels.clone()));

        start_event_listener(broadcaster.clone(), rx.clone());

        App::new()
            .wrap(Logger::default())
            .wrap(DefaultHeaders::new().add(("Server", title.as_str())))
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
            )
    });

    if let Some(tls_config) = tls_config {
        server.bind_rustls_0_23(&bind_address, tls_config)?
    } else {
        server.bind(&bind_address)?
    }
    .workers(num_workers)
    .run()
    .await
    .map_err(Error::from)
}

//
// Logger
//
fn init_logger(verbose: u8) {
    use env_logger::Env;
    use std::io::Write;

    let mut builder = env_logger::Builder::from_env(
        Env::new()
            .filter("PG_EVENT_SERVER_LOG")
            .default_filter_or("info"),
    );

    if verbose > 0 {
        builder.format(|buf, record| {
            writeln!(
                buf,
                "{} {:5} [{}] {}",
                buf.timestamp(),
                record.level(),
                record.module_path().unwrap_or_default(),
                record.args()
            )
        });
    } else {
        builder.format(|buf, record| {
            writeln!(
                buf,
                "{} {:5} {}",
                buf.timestamp(),
                record.level(),
                record.args()
            )
        });
    }

    match verbose {
        1 => builder.filter_level(LevelFilter::Debug),
        _ if verbose > 1 => builder.filter_level(LevelFilter::Trace),
        _ => &mut builder,
    }
    .init();
}

#[cfg(test)]
mod tests;
