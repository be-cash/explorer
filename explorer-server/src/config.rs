use std::net::SocketAddr;

use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Modes {
    Production,
    Development,
}

#[derive(Deserialize)]
pub struct Config {
    pub mode: Modes,
    pub index_database: String,
    pub host: SocketAddr,
}

pub fn load_config(config_string: &str) -> Result<Config> {
    let config: Config = toml::from_str(config_string).unwrap();
    return Ok(config);
}
