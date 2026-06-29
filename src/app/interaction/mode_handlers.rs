use crate::app::core::{AppState, ModeKind};

pub trait ModeHandler {
    fn handle(&mut self, state: &mut AppState);
}

pub struct LightReadingHandler;
impl ModeHandler for LightReadingHandler {
    fn handle(&mut self, state: &mut AppState) {
        if let Some(tab) = state.current_tab_mut() {
            if tab.modes.active == ModeKind::LightReading {
                let _ = &tab.modes.reading;
            }
        }
    }
}

pub struct DeepReadingHandler;
impl ModeHandler for DeepReadingHandler {
    fn handle(&mut self, state: &mut AppState) {
        if let Some(tab) = state.current_tab_mut() {
            if tab.modes.active == ModeKind::DeepReading {
                let _ = &tab.modes.annotate;
            }
        }
    }
}
