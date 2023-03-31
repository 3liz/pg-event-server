//!
//! Handle passfile
//!
use crate::{Config, Error, Result};
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tokio_postgres::config::Host;

/// Look for passfile
/// First check the environment variable PGPASSFILE
/// then check in $HOME/.pgpass
fn get_passfile() -> Option<PathBuf> {
    std::env::var("PGPASSFILE")
        .map(|path| Path::new(&path).into())
        .or_else(|_| std::env::var("HOME").map(|path| Path::new(&path).join(".pgpass")))
        .ok()
}

/// Match host value
fn match_host(value: &str, config: &Config) -> Result<bool> {
    Ok(value == "*"
        || config.get_hosts().iter().any(|host| match host {
            Host::Tcp(s) => value == s,
            Host::Unix(p) => p == Path::new(value),
        }))
}

fn match_port(value: &str, config: &Config) -> Result<bool> {
    Ok(value == "*" || {
        let port: u16 = value.parse().map_err(|_| Error::PassfileParseError)?;
        config.get_ports().iter().any(|p| *p == port)
    })
}

fn match_dbname(value: &str, config: &Config) -> Result<bool> {
    Ok(value == "*" || config.get_dbname() == Some(value))
}

fn match_username(value: &str, config: &Config) -> Result<bool> {
    Ok(value == "*" || config.get_user() == Some(value))
}

fn get_password<'a>(line: &'a str, config: &Config) -> Result<Option<&'a str>> {
    let mut parts = line.split(':');
    if match_host(parts.next().ok_or(Error::PassfileParseError)?, config)?
        && match_port(parts.next().ok_or(Error::PassfileParseError)?, config)?
        && match_dbname(parts.next().ok_or(Error::PassfileParseError)?, config)?
        && match_username(parts.next().ok_or(Error::PassfileParseError)?, config)?
    {
        Ok(Some(parts.next().ok_or(Error::PassfileParseError)?))
    } else {
        Ok(None)
    }
}

use std::ops::ControlFlow;

/// Get Password from passfile
pub(crate) fn get_password_from_passfile(config: &mut Config) -> Result<()> {
    if let Some(path) = get_passfile() {
        // Check permission
        let path = path.as_path();

        if fs::metadata(path)?.permissions().mode() & 0o7777 != 0o600 {
            return Err(Error::InvalidPassFileMode);
        }

        let file = fs::File::open(path)?;
        // Read all lines in pass file
        match BufReader::new(file)
            .lines()
            .try_for_each(|line| match line {
                Err(err) => ControlFlow::Break(Err(Error::from(err))),
                Ok(l) => {
                    let l = l.as_str().trim();
                    if l.is_empty() || l.starts_with('#') {
                        ControlFlow::Continue(())
                    } else {
                        match get_password(l, config) {
                            Err(err) => ControlFlow::Break(Err(err)),
                            Ok(Some(pwd)) => {
                                config.password(pwd);
                                ControlFlow::Break(Ok(()))
                            }
                            Ok(None) => ControlFlow::Continue(()),
                        }
                    }
                }
            }) {
            ControlFlow::Break(err) => err,
            _ => Ok(()),
        }
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;

    #[test]
    fn parse_passfile() {
        std::env::set_var(
            "PGPASSFILE",
            Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
                .join("fixtures")
                .as_path()
                .join("passfile.conf")
                .to_str()
                .unwrap(),
        );

        let mut conf = Config::new();
        conf.host("db.bar.com").dbname("bardb").user("bar");

        get_password_from_passfile(&mut conf).unwrap();
        assert_eq!(conf.get_password(), Some("barpwd".as_bytes()));

        let mut conf = Config::new();
        conf.host("/var/run/postgresql").dbname("foodb").user("foo");

        get_password_from_passfile(&mut conf).unwrap();
        assert_eq!(conf.get_password(), Some("foopwd".as_bytes()));

        let mut conf = Config::new();
        conf.host("/var/run/postgresql").dbname("bazdb").user("foo");

        get_password_from_passfile(&mut conf).unwrap();
        assert_eq!(conf.get_password(), None);
    }
}
