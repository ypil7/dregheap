use serde::Deserialize;
use std::time::Duration;

use crate::errors::{Error, Result};

#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_read_timeout_ms")]
    pub read_timeout_ms: u64,

    #[serde(default = "default_write_timeout_ms")]
    pub write_timeout_ms: u64,

    #[serde(default = "default_idle_connection_ttl")]
    pub idle_connection_ttl: u64,

    #[serde(default = "default_max_connection_lifetime")]
    pub max_connection_lifetime: u64,
}

pub fn load_config() -> Result<Config> {
    let config = envy::prefixed("DREG_")
        .from_env::<Config>()
        .map_err(|_e| Error::Custom("".to_string()))?;
    Ok(config)
}

fn default_port() -> u16 {
    6767
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.port < 1024 {
            return Err(Error::Custom(format!(
                "Invalid port number {} port number must be 1025 or greater",
                self.port
            )));
        }
        if self.read_timeout_ms == 0 {
            return Err(Error::Custom(
                "Invalid read timeout: must be greater than 0ms".to_string(),
            ));
        }
        if self.write_timeout_ms == 0 {
            return Err(Error::Custom(
                "Invalid write timeout: must be greater than 0ms".to_string(),
            ));
        }
        Ok(())
    }

    pub fn read_timeout(&self) -> Duration {
        Duration::from_millis(self.read_timeout_ms)
    }

    pub fn write_timeout(&self) -> Duration {
        Duration::from_millis(self.write_timeout_ms)
    }

    pub fn idle_connection_ttl(&self) -> Duration {
        Duration::from_millis(self.idle_connection_ttl)
    }

    pub fn max_connection_lifetime(&self) -> Duration {
        Duration::from_millis(self.max_connection_lifetime)
    }
}

fn default_read_timeout_ms() -> u64 {
    10_000
}

fn default_write_timeout_ms() -> u64 {
    10_000
}

fn default_idle_connection_ttl() -> u64 {
    3 * 60 * 1000
}

fn default_max_connection_lifetime() -> u64 {
    30 * 60 * 1000
}
