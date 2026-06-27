use crate::app::core::{AppState, Mode};

pub struct InputRouter;

impl InputRouter {
    pub fn handle_key(state: &mut AppState, key: &str) {
        if let Some(tab) = state.current_tab_mut() {
            match key {
                "r" | "R" => tab.mode = Mode::reading(),
                "a" | "A" => tab.mode = Mode::auto(),
                "n" | "N" => tab.mode = Mode::annotate(),
                _ => {}
            }
        }
    }

    pub fn handle_click(state: &mut AppState, pos: [f32; 2]) {
        if let Some(tab) = state.current_tab_mut() {
            if let Mode::Annotate(ref mut an) = tab.mode {
                an.stroke_points.push(pos);
            }
        }
    }
}
