//! Hub for Postgres notification to SSE broadcast
//!
use log::LevelFilter;

mod config;
mod errors;
mod landingpage;
mod services;

use services::subscribe::Broadcaster;

use errors::{Error, Result};
use std::path::Path;

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

    HttpServer::new(move || {
        let app = App::new()
            .wrap(Logger::default())
            .service(
                web::resource("/")
                    .name("landing_page")
                    .route(web::get().to(landingpage::handler)),
            )
            .service(
                web::resource("/subscribe/{id}")
                    .app_data(web::Data::new(Broadcaster::from_config(&conf.broadcast)))
                    .route(web::get().to(Broadcaster::handler)),
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

    // Setup logger, bypasse environment variables
    let mut builder = env_logger::Builder::from_env(Env::default().default_filter_or("info"));

    if verbose {
        builder.filter_level(LevelFilter::Trace);
        eprintln!("Verbose mode on");
    }
    builder.init();
}
