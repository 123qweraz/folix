use crate::app::engines::Document;
use super::mode_system::{TabModes, ViewMode};
use super::feature_system::FeatureSystem;
use super::shortcuts::{ShortcutMap, default_shortcuts};
use std::sync::Arc;
use parking_lot::Mutex;

#[derive(Clone)]
pub enum TabContent {
    Document,
    NewTab,
    Settings,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    pub toolbar_icon_size: f32,
    pub show_toolbar: bool,
    pub background_color: [u8; 4],
    pub dark_mode: bool,
    pub shortcuts: ShortcutMap,
    #[serde(skip)]
    pub editing_shortcut: Option<usize>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            toolbar_icon_size: 16.0,
            show_toolbar: true,
            background_color: [255, 255, 255, 255],
            dark_mode: false,
            shortcuts: default_shortcuts(),
            editing_shortcut: None,
        }
    }
}

pub struct OpenTab {
    pub content: TabContent,
    pub document: Option<Arc<Mutex<Box<dyn Document>>>>,
    pub path: Option<String>,
    pub modes: TabModes,
    pub book_id: Option<String>,
}

impl OpenTab {
    pub fn title(&self) -> String {
        match self.content {
            TabContent::Settings => "⚙ Settings".to_string(),
            _ => match &self.path {
                Some(p) => std::path::Path::new(p)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled")
                    .to_string(),
                None => "+ New Tab".to_string(),
            },
        }
    }

    pub fn is_new_tab(&self) -> bool {
        matches!(self.content, TabContent::NewTab)
    }

    pub fn is_settings_tab(&self) -> bool {
        matches!(self.content, TabContent::Settings)
    }

    pub fn has_document(&self) -> bool {
        matches!(self.content, TabContent::Document) && self.document.is_some()
    }
}

pub struct AppState {
    pub tabs: Vec<OpenTab>,
    pub active_tab: usize,
    pub feature_system: FeatureSystem,
    pub ui_visible: bool,
    pub settings: AppSettings,
}

impl AppState {
    pub fn new() -> Self {
        let mut state = Self {
            tabs: Vec::new(),
            active_tab: 0,
            feature_system: FeatureSystem::new(),
            ui_visible: false,
            settings: AppSettings::default(),
        };
        state.add_new_tab();
        state
    }

    pub fn add_tab(&mut self, path: String, document: Arc<Mutex<Box<dyn Document>>>) -> usize {
        let idx = self.tabs.len();
        let mut modes = TabModes::new();
        modes.reading.view_mode = if document.lock().supports_image() {
            ViewMode::Image
        } else {
            ViewMode::Text
        };
        self.tabs.push(OpenTab {
            content: TabContent::Document,
            document: Some(document),
            path: Some(path),
            modes,
            book_id: None,
        });
        self.active_tab = idx;
        idx
    }

    pub fn add_new_tab(&mut self) -> usize {
        let idx = self.tabs.len();
        self.tabs.push(OpenTab {
            content: TabContent::NewTab,
            document: None,
            path: None,
            modes: TabModes::new(),
            book_id: None,
        });
        self.active_tab = idx;
        idx
    }

    pub fn add_settings_tab(&mut self) -> usize {
        // Reuse existing settings tab if already open
        for (i, tab) in self.tabs.iter().enumerate() {
            if tab.is_settings_tab() {
                self.active_tab = i;
                return i;
            }
        }
        let idx = self.tabs.len();
        self.tabs.push(OpenTab {
            content: TabContent::Settings,
            document: None,
            path: None,
            modes: TabModes::new(),
            book_id: None,
        });
        self.active_tab = idx;
        idx
    }

    pub fn close_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        self.tabs.remove(idx);
        if self.tabs.is_empty() {
            self.add_new_tab();
        } else if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if idx < self.active_tab {
            self.active_tab -= 1;
        }
    }

    pub fn current_tab(&self) -> Option<&OpenTab> {
        self.tabs.get(self.active_tab)
    }

    pub fn current_tab_mut(&mut self) -> Option<&mut OpenTab> {
        self.tabs.get_mut(self.active_tab)
    }
}
