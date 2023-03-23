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
mod services;

use services::subscribe::Broadcaster;

use errors::{Error, Result};
use std::path::Path;
use std::rc::Rc;
use std::cell::RefCell;
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

use tokio::sync::watch::{self, Sender, Receiver};

fn broadcast_events(tx: Sender<Event>) {
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

// Start listening to incoming events and broadcast 
// it 
fn listen_for_events(bc: Rc<Broadcaster>, mut rx: Receiver<Event>) {
    actix_web::rt::spawn(async move {
        log::info!("Event listener started");
        while rx.changed().await.is_ok() {
            let ev = rx.borrow();
            log::info!("EVENT({}) CHANGED", ev.id);
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

    let conf = config::read_config(&Path::new(&args.config))?;
    init_logger(args.verbose);

    let listen = conf.server.listen.clone();

    eprintln!("Starting subscription server on: {listen}");

    let (tx, rx) = watch::channel(Event::default());
    
    broadcast_events(tx);

    HttpServer::new(move || {

        let broadcaster = Rc::new(Broadcaster::new(&conf.broadcast));

        listen_for_events(broadcaster.clone(), rx.clone());     

        let app = App::new()
            .wrap(Logger::default())
            .service(
                web::resource("/")
                    .name("landing_page")
                    .route(web::get().to(landingpage::handler)),
            )
            .service(
                web::scope("/events")
                    .app_data(web::Data::new(broadcaster.clone()))
                    .route("/subscribe/{id}", web::get().to(Broadcaster::do_subscribe))
                    //.route("/notify/{id}/{event}", web::post().to(Broadcaster::do_notify)),
            );

        app
    })
    .bind(listen)?
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
