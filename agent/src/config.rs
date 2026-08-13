use anyhow::Result;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server_url: Option<String>,
    pub host: Option<String>,
    pub token: Option<String>,
    pub systemd_units: Option<Vec<String>>,
}

impl Config {
    pub fn load(path: Option<&str>) -> Result<Self> {
        match path {
            Some(path) => {
                let content = fs::read_to_string(path)?;
                Ok(toml::from_str(&content)?)
            }
            None => Ok(Self {
                server_url: None,
                host: None,
                token: None,
                systemd_units: None,
            }),
        }
    }
}
