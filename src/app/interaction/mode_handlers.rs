use crate::app::core::{AppState, Mode};

pub trait ModeHandler {
    fn handle(&mut self, state: &mut AppState);
}

pub struct ReadingHandler;
impl ModeHandler for ReadingHandler {
    fn handle(&mut self, state: &mut AppState) {
        if let Mode::Reading(ref mut rs) = state.mode {
            let _ = rs;
        }
    }
}

pub struct AutoHandler;
impl ModeHandler for AutoHandler {
    fn handle(&mut self, state: &mut AppState) {
        if let Mode::Auto(ref mut aut) = state.mode {
            let _ = aut;
        }
    }
}

pub struct AnnotateHandler;
impl ModeHandler for AnnotateHandler {
    fn handle(&mut self, state: &mut AppState) {
        if let Mode::Annotate(ref mut an) = state.mode {
            let _ = an;
        }
    }
}
