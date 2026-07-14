use serde::Deserialize;
use std::error::Error;

use dregheap::errors::{Result};

use crate::errors;

#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(default = "default_port")]
    pub port: u16,
}

pub fn load_config() -> Result<Config> {
    let config = envy::prefixed("DREG_").from_env::<Config>()?;
    Ok(config)
}

fn default_port() -> u16 {
    6767
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.port < 1024 {
            return Err(format!(
                "Invalid port number {} port number must be 1025 or greater",
                self.port
            ));
        }
        Ok(())
    }
}
