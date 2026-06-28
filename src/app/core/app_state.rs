use crate::app::engines::Document;
use super::mode_system::TabModes;
use super::feature_system::FeatureSystem;
use std::sync::Arc;
use parking_lot::Mutex;

pub struct OpenTab {
    pub document: Option<Arc<Mutex<Box<dyn Document>>>>,
    pub path: Option<String>,
    pub modes: TabModes,
}

impl OpenTab {
    pub fn title(&self) -> String {
        match &self.path {
            Some(p) => std::path::Path::new(p)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string(),
            None => "+ New Tab".to_string(),
        }
    }

    pub fn is_new_tab(&self) -> bool {
        self.document.is_none()
    }
}

pub struct AppState {
    pub tabs: Vec<OpenTab>,
    pub active_tab: usize,
    pub feature_system: FeatureSystem,
    pub ui_visible: bool,
}

impl AppState {
    pub fn new() -> Self {
        let mut state = Self {
            tabs: Vec::new(),
            active_tab: 0,
            feature_system: FeatureSystem::new(),
            ui_visible: false,
        };
        state.add_new_tab();
        state
    }

    pub fn add_tab(&mut self, path: String, document: Arc<Mutex<Box<dyn Document>>>) -> usize {
        let idx = self.tabs.len();
        let mut modes = TabModes::new();
        modes.reading.view_mode = if document.lock().supports_image() {
            super::mode_system::ViewMode::Image
        } else {
            super::mode_system::ViewMode::Text
        };
        self.tabs.push(OpenTab {
            document: Some(document),
            path: Some(path),
            modes,
        });
        self.active_tab = idx;
        idx
    }

    pub fn add_new_tab(&mut self) -> usize {
        let idx = self.tabs.len();
        self.tabs.push(OpenTab {
            document: None,
            path: None,
            modes: TabModes::new(),
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
