use anyhow::Result;
use serde::{Deserialize, Serialize};
use simple_home_dir::home_dir;
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = ".abu/last_settings.json";

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub last_my_privkey: Option<PathBuf>,
    pub last_my_pubkey: Option<PathBuf>,
    pub last_peer_pubkey: Option<PathBuf>,
    pub last_export_dir: Option<PathBuf>,
}

impl Config {
    pub fn load() -> Self {
        if let Some(home) = home_dir() {
            let path = home.join(CONFIG_FILE);
            if let Ok(data) = fs::read_to_string(path) {
                if let Ok(conf) = serde_json::from_str(&data) {
                    return conf;
                }
            }
        }
        Config::default()
    }

    pub fn save(&self) -> Result<()> {
        if let Some(home) = home_dir() {
            let path = home.join(CONFIG_FILE);
            let data = serde_json::to_string_pretty(self)?;
            fs::write(path, data)?;
        }
        Ok(())
    }
}