use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub port: u16,
}

pub fn load_config() -> Result<Config, envy::Error> {
    let config = envy::prefixed("DREG_").from_env::<Config>()?;
    Ok(config)
}
