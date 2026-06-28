use crate::app::core::{AppState, ModeKind};

pub trait ModeHandler {
    fn handle(&mut self, state: &mut AppState);
}

pub struct ReadingHandler;
impl ModeHandler for ReadingHandler {
    fn handle(&mut self, state: &mut AppState) {
        if let Some(tab) = state.current_tab_mut() {
            if tab.modes.active == ModeKind::Reading {
                let _ = &tab.modes.reading;
            }
        }
    }
}

pub struct AutoHandler;
impl ModeHandler for AutoHandler {
    fn handle(&mut self, state: &mut AppState) {
        if let Some(tab) = state.current_tab_mut() {
            if tab.modes.active == ModeKind::Auto {
                let _ = &tab.modes.auto;
            }
        }
    }
}

pub struct AnnotateHandler;
impl ModeHandler for AnnotateHandler {
    fn handle(&mut self, state: &mut AppState) {
        if let Some(tab) = state.current_tab_mut() {
            if tab.modes.active == ModeKind::Annotate {
                let _ = &tab.modes.annotate;
            }
        }
    }
}
