use crate::app::core::app_state::AppSettings;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub fn config_path() -> PathBuf {
    // Follows XDG Base Directory spec on Linux, standard platform conventions on macOS/Windows
    let base = if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(dir)
    } else if let Some(dir) = std::env::var_os("HOME") {
        PathBuf::from(dir).join(".config")
    } else {
        PathBuf::from(".")
    };
    base.join("folix").join("folix.conf")
}

/// Standard data directory for folix (database, etc.)
pub fn data_dir() -> PathBuf {
    let base = if let Some(dir) = std::env::var_os("XDG_DATA_HOME") {
        PathBuf::from(dir)
    } else if let Some(dir) = std::env::var_os("HOME") {
        PathBuf::from(dir).join(".local").join("share")
    } else {
        PathBuf::from(".")
    };
    base.join("folix")
}

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
        let path = config_path();
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&path).ok()?;
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
            let path = config_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&path, content);
        }
    }
}
