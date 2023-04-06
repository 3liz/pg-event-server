//!
//! Check the postgres configuration
//!
//! Output the initialized configuration.
//!
//! This enable to check the configuration
//! settings given from connection string
//! environment and service files.
//!
use pg_client_config::{load_config, Result};

fn main() -> Result<()> {
    let arg = std::env::args().nth(1);
    println!("Checking postgres config from: {arg:?}");
    let config = load_config(arg.as_deref())?;
    println!("{config:#?}");
    Ok(())
}
