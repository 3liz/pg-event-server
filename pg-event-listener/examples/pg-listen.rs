//!
//! Listen for pg event
//!
use pg_config::{load_pg_config, Result};
use pg_event_listener::PgEventListener;

use clap::Parser;

#[derive(Parser)]
#[command(author, version="0.1", about="Postgres event listener example", long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Path to configuration file
    config: String,
    /// Event to listen to 
    event: String,
}


#[tokio::main]
async fn main() -> Result<()> {

    let args = Cli::parse();
    let config = load_pg_config(Some(&args.config))?;

    println!("Starting event listener");
    let mut evl = PgEventListener::connect(config).await?;

    println!("CONNECTED({})\n{:#?}", 
        evl.session_pid().await?,
        evl.config()
    );

    evl.listen(&args.event).await?;

    while let Some(event) = evl.recv().await {
        println!("===> RECEIVED EVENT");
        println!{"{event:#?}"}; 
    }

    Ok(())
}

