use log::debug;
use serde::Deserialize;
use std::env;
use std::error::Error;
use std::fs;

use crate::api::MagmaConfig;
use crate::node::LNDConfig;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub loop_interval: Option<u64>,
    pub lnd: LNDConfig,
    pub magma: MagmaConfig,
}

pub fn load() -> Result<Config, Box<dyn Error>> {
    let path = env::var("CONFIG_PATH").unwrap_or_else(|_| "config.yaml".to_string());
    debug!("Loading config from {}", fs::canonicalize(&path)?.display());

    let contents = fs::read_to_string(path)?;
    let config: Config = serde_yaml::from_str(&contents)?;
    debug!("Config loaded: {:?}", config);
    Ok(config)
}
