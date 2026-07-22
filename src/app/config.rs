use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application settings persisted to disk.
#[derive(Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub toolbar_icon_size: f32,
    pub show_toolbar_nav: bool,
    pub show_toolbar_view: bool,
    pub show_toolbar_page: bool,
    pub show_toolbar_auto: bool,
    pub show_toolbar_edit: bool,
    pub background_color: [u8; 4],
    pub reader_bg_color: [u8; 4],
    pub dark_mode: bool,
    pub scroll_speed: f32,
    pub mo_yu_speed: f32,
    pub shortcuts: crate::app::core::shortcuts::ShortcutMap,
    pub language: String,
    #[serde(skip)]
    pub editing_shortcut: Option<usize>,
    pub reading_font_size: f32,
    pub reading_line_height: f32,
    pub reading_margin_h: f32,
    pub reading_max_text_width: f32,
    pub reading_side_margin_pct: f32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            toolbar_icon_size: 16.0,
            show_toolbar_nav: true,
            show_toolbar_view: true,
            show_toolbar_page: true,
            show_toolbar_auto: true,
            show_toolbar_edit: true,
            background_color: [255, 255, 255, 255],
            reader_bg_color: [235, 235, 238, 255],
            dark_mode: false,
            scroll_speed: 800.0,
            mo_yu_speed: 1.5,
            shortcuts: crate::app::core::shortcuts::default_shortcuts(),
            language: "zh-CN".into(),
            editing_shortcut: None,
            reading_font_size: 16.0,
            reading_line_height: 1.4,
            reading_margin_h: 16.0,
            reading_max_text_width: 720.0,
            reading_side_margin_pct: 0.25,
        }
    }
}

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
