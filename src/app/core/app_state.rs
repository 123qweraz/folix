use crate::app::engines::Document;
use super::mode_system::{Mode, ModeController};
use super::feature_system::FeatureSystem;
use std::sync::Arc;
use parking_lot::Mutex;

pub struct AppState {
    pub mode: Mode,
    pub document: Option<Arc<Mutex<Box<dyn Document>>>>,
    pub document_path: Option<String>,
    pub feature_system: FeatureSystem,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            mode: Mode::reading(),
            document: None,
            document_path: None,
            feature_system: FeatureSystem::new(),
        }
    }
}

impl ModeController for AppState {
    fn switch(&mut self, mode: Mode) {
        self.mode = mode;
    }

    fn current(&self) -> &Mode {
        &self.mode
    }
}
