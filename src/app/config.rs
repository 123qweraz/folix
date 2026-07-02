use crate::app::core::app_state::AppSettings;
use serde::{Deserialize, Serialize};
use std::path::Path;

const CONFIG_PATH: &str = "./folix.conf";

#[derive(Serialize, Deserialize, Clone)]
pub struct RecentFile {
    pub path: String,
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ConfigData {
    pub settings: AppSettings,
    #[serde(default = "default_recent_files")]
    pub recent_files: Vec<RecentFile>,
}

fn default_recent_files() -> Vec<RecentFile> {
    vec![]
}

impl ConfigData {
    pub fn load() -> Option<Self> {
        if !Path::new(CONFIG_PATH).exists() {
            return None;
        }
        let content = std::fs::read_to_string(CONFIG_PATH).ok()?;
        // Try new format (Vec<RecentFile>), then fall back to old Vec<String>
        if let Ok(data) = serde_json::from_str::<ConfigData>(&content) {
            return Some(data);
        }
        // Legacy compat: try old { settings, recent_files: Vec<String> }
        #[derive(Deserialize)]
        struct OldConfig {
            settings: AppSettings,
            recent_files: Vec<String>,
        }
        if let Ok(old) = serde_json::from_str::<OldConfig>(&content) {
            return Some(ConfigData {
                settings: old.settings,
                recent_files: old.recent_files.into_iter()
                    .map(|p| RecentFile { path: p, pinned: false })
                    .collect(),
            });
        }
        None
    }

    pub fn save(&self) {
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(CONFIG_PATH, content);
        }
    }
}
