use crate::app::core::{AppState, Mode};

pub trait ModeHandler {
    fn handle(&mut self, state: &mut AppState);
}

pub struct ReadingHandler;
impl ModeHandler for ReadingHandler {
    fn handle(&mut self, state: &mut AppState) {
        if let Some(tab) = state.current_tab_mut() {
            if let Mode::Reading(ref mut rs) = tab.mode {
                let _ = rs;
            }
        }
    }
}

pub struct AutoHandler;
impl ModeHandler for AutoHandler {
    fn handle(&mut self, state: &mut AppState) {
        if let Some(tab) = state.current_tab_mut() {
            if let Mode::Auto(ref mut aut) = tab.mode {
                let _ = aut;
            }
        }
    }
}

pub struct AnnotateHandler;
impl ModeHandler for AnnotateHandler {
    fn handle(&mut self, state: &mut AppState) {
        if let Some(tab) = state.current_tab_mut() {
            if let Mode::Annotate(ref mut an) = tab.mode {
                let _ = an;
            }
        }
    }
}
