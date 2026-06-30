use crate::app::core::app_state::AppSettings;
use serde::{Deserialize, Serialize};
use std::path::Path;

const CONFIG_PATH: &str = "./folix.conf";

#[derive(Serialize, Deserialize, Clone)]
pub struct ConfigData {
    pub settings: AppSettings,
    pub recent_files: Vec<String>,
}

impl ConfigData {
    pub fn load() -> Option<Self> {
        if !Path::new(CONFIG_PATH).exists() {
            return None;
        }
        let content = std::fs::read_to_string(CONFIG_PATH).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&self) {
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(CONFIG_PATH, content);
        }
    }
}
