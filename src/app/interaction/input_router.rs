use crate::app::core::{AppState, Mode};
use crate::app::core::mode_system::ModeController;

pub struct InputRouter;

impl InputRouter {
    pub fn handle_key(state: &mut AppState, key: &str) {
        match key {
            "r" | "R" => state.switch(Mode::reading()),
            "a" | "A" => state.switch(Mode::auto()),
            "n" | "N" => state.switch(Mode::annotate()),
            _ => {}
        }
    }

    pub fn handle_click(state: &mut AppState, _pos: [f32; 2]) {
        match state.mode {
            Mode::Annotate(ref mut an) => {
                an.stroke_points.push(_pos);
            }
            _ => {}
        }
    }
}
