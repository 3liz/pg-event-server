//!
//! Unit tests
//!
use env_logger;
use std::{env, path::Path, sync::Once};

static INIT: Once = Once::new();

pub fn setup() {
    // Init setup
    INIT.call_once(|| {
        env_logger::init();
    });
}

macro_rules! confdir {
    ($name:expr) => {
        Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("tests")
            .as_path()
            .join($name)
            .as_path()
    };
}

pub(crate) use confdir;
