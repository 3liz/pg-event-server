//!
//! Listen for pg event
//!
use pg_config::{load_pg_config, Result};
use pg_event_listener::PgEventListener;

use clap::{ArgAction, Parser};

#[derive(Parser)]
#[command(author, version="0.1", about="Postgres event listener example", long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Path to configuration file
    config: String,
    /// Event to listen to
    event: String,
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();
    let config = load_pg_config(Some(&args.config))?;

    init_logger(args.verbose);

    let mut evl = PgEventListener::connect(config).await?;

    println!("CONNECTED({})\n{:#?}", evl.session_pid(), evl.config());

    evl.listen(&args.event).await?;

    while let Some(event) = evl.recv().await {
        println!("===> RECEIVED EVENT");
        println! {"{event:#?}"};
    }

    Ok(())
}

//
// Logger
//
fn init_logger(verbose: u8) {
    use env_logger::Env;
    use log::LevelFilter;

    let mut builder = env_logger::Builder::from_env(Env::default().default_filter_or("info"));

    match verbose {
        1 => builder.filter_level(LevelFilter::Debug),
        _ if verbose > 1 => builder.filter_level(LevelFilter::Trace),
        _ => &mut builder,
    }
    .init();
}
