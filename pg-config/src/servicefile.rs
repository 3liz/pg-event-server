//!
//! Handle service file
//!
use std::path::{Path, PathBuf};

find_sysconf_servic_file(

fn find_service_file() -> Option<PathBuf> {
    std::env::var("PGSERVICEFILE")
    .and_then(|path| Path::new(&path).as_path())
    .or_else(|_| 
        Ok(Path::new(std::env::var("HOME")?)
            .join(".pg_service.conf")
            .as_path())
    ).ok()
}

