//!
//! Check the postgres configuration
//!
//! Output the initialized configuration.
//!
//! This enable to check the configuration
//! settings given from connection string
//! environment and service files.
//!
use pg_config::{load_pg_config, Result};

fn main() -> Result<()> {
    let arg = std::env::args().nth(1);
    println!("Checking postgres config from: {arg:?}");
    let config = load_pg_config(arg.as_deref())?;
    println!("{config:#?}");
    Ok(())
}
