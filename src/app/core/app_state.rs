use crate::app::engines::DocumentHandle;
use super::mode_system::{TabModes, ViewMode};
use super::feature_system::FeatureSystem;
use super::shortcuts::{ShortcutMap, default_shortcuts};
use super::pdf_toolbox::PdfToolboxState;
use std::sync::Arc;
use parking_lot::Mutex;

#[derive(Clone)]
pub enum TabContent {
    Document,
    NewTab,
    Settings,
    PdfToolbox(PdfToolboxState),
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    pub toolbar_icon_size: f32,
    pub show_toolbar_nav: bool,
    pub show_toolbar_view: bool,
    pub show_toolbar_page: bool,
    pub show_toolbar_auto: bool,
    pub show_toolbar_annotate: bool,
    pub show_toolbar_edit: bool,
    pub background_color: [u8; 4],
    pub dark_mode: bool,
    pub scroll_speed: f32,
    pub mo_yu_speed: f32,
    #[serde(default = "super::shortcuts::default_shortcuts")]
    pub shortcuts: ShortcutMap,
    pub language: String,
    #[serde(skip)]
    pub editing_shortcut: Option<usize>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            toolbar_icon_size: 16.0,
            show_toolbar_nav: true,
            show_toolbar_view: true,
            show_toolbar_page: true,
            show_toolbar_auto: true,
            show_toolbar_annotate: true,
            show_toolbar_edit: true,
            background_color: [255, 255, 255, 255],
            dark_mode: false,
            scroll_speed: 800.0,
            mo_yu_speed: 1.5,
            shortcuts: default_shortcuts(),
            language: "zh-CN".into(),
            editing_shortcut: None,
        }
    }
}

pub struct OpenTab {
    pub content: TabContent,
    pub document: Option<Arc<Mutex<DocumentHandle>>>,
    pub path: Option<String>,
    pub modes: TabModes,
    pub book_id: Option<String>,
}

impl OpenTab {
    pub fn title(&self, lang: &str) -> String {
        match &self.content {
            TabContent::Settings => crate::app::i18n::tr(lang, "⚙ Settings").to_string(),
            TabContent::PdfToolbox(_) => crate::app::i18n::tr(lang, "📄 PDF Tools").to_string(),
            _ => match &self.path {
                Some(p) => std::path::Path::new(p)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_else(|| crate::app::i18n::tr(lang, "Untitled"))
                    .to_string(),
                None => crate::app::i18n::tr(lang, "+ New Tab").to_string(),
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

    pub fn is_pdf_toolbox(&self) -> bool {
        matches!(self.content, TabContent::PdfToolbox(_))
    }

    pub fn pdf_toolbox_mut(&mut self) -> Option<&mut PdfToolboxState> {
        match &mut self.content {
            TabContent::PdfToolbox(state) => Some(state),
            _ => None,
        }
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
    /// Translate a string key according to the current language setting.
    pub fn tr(&self, text: &'static str) -> &'static str {
        crate::app::i18n::tr(&self.settings.language, text)
    }

    pub fn new() -> Self {
        let mut state = Self {
            tabs: Vec::new(),
            active_tab: 0,
            feature_system: FeatureSystem::new(),
            ui_visible: true,
            settings: AppSettings::default(),
        };
        state.add_new_tab();
        state
    }

    pub fn add_tab(&mut self, path: String, document: Arc<Mutex<DocumentHandle>>) -> usize {
        let idx = self.tabs.len();
        let mut modes = TabModes::new();
        let is_fixed = document.lock().is_fixed();
        modes.reading.view_mode = if is_fixed {
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

    pub fn add_pdf_toolbox_tab(&mut self) -> usize {
        for (i, tab) in self.tabs.iter().enumerate() {
            if tab.is_pdf_toolbox() {
                self.active_tab = i;
                return i;
            }
        }
        let idx = self.tabs.len();
        self.tabs.push(OpenTab {
            content: TabContent::PdfToolbox(PdfToolboxState::new()),
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
