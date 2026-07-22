use crate::app::core::document_manager::DocumentManager;
use crate::slint_app::reflow_viewer::ReflowViewerState;
use crate::slint_app::pdf_viewer::PdfViewerState;

pub struct Tab {
    pub title: String,
    pub path: String,
    pub content: TabContent,
}

pub enum TabContent {
    Reflow(ReflowViewerState),
    Pdf(PdfViewerState),
}

impl Tab {
    pub fn title_for_display(&self) -> String {
        let name = match &self.content {
            TabContent::Reflow(state) => state.current_title(),
            TabContent::Pdf(state) => state.document_title(),
        };
        if !name.is_empty() {
            return name;
        }
        std::path::Path::new(&self.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Untitled".to_string())
    }

    pub fn is_pdf(&self) -> bool {
        matches!(self.content, TabContent::Pdf(_))
    }
}

pub struct AppState {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
}

impl AppState {
    pub fn new() -> Self {
        Self { tabs: Vec::new(), active_tab: 0 }
    }

    pub fn open_file(&mut self, path: &str) -> Result<(), String> {
        let handle = DocumentManager::open(path).ok_or_else(|| "Failed to open file".to_string())?;
        let handle_guard = handle.lock();

        let is_pdf = handle_guard.is_fixed();
        let title = handle_guard.title();
        drop(handle_guard);

        let content = if is_pdf {
            let mut state = PdfViewerState::new();
            state.load_handle(handle.clone());
            TabContent::Pdf(state)
        } else {
            let mut state = ReflowViewerState::new();
            state.load_handle(handle.clone());
            TabContent::Reflow(state)
        };

        let tab = Tab {
            title,
            path: path.to_string(),
            content,
        };

        self.active_tab = self.tabs.len();
        self.tabs.push(tab);

        Ok(())
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        if self.active_tab < self.tabs.len() {
            Some(&self.tabs[self.active_tab])
        } else {
            None
        }
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        if self.active_tab < self.tabs.len() {
            Some(&mut self.tabs[self.active_tab])
        } else {
            None
        }
    }

    pub fn switch_to_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active_tab = idx;
        }
    }

    pub fn close_tab(&mut self, idx: usize) -> bool {
        if idx >= self.tabs.len() {
            return false;
        }
        self.tabs.remove(idx);
        if self.tabs.is_empty() {
            self.active_tab = 0;
            return true; // no tabs left
        }
        if self.active_tab >= idx && self.active_tab > 0 {
            self.active_tab -= 1;
        }
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        !self.tabs.is_empty()
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn current_title(&self) -> String {
        self.active_tab().map(|t| t.title_for_display()).unwrap_or_default()
    }
}
