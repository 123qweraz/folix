use crate::app::core::{AppState, ModeKind};

pub struct InputRouter;

impl InputRouter {
    pub fn handle_key(state: &mut AppState, key: &str) {
        if let Some(tab) = state.current_tab_mut() {
            match key {
                "r" | "R" => tab.modes.switch_to(ModeKind::Reading),
                "a" | "A" => tab.modes.switch_to(ModeKind::Auto),
                "n" | "N" => tab.modes.switch_to(ModeKind::Annotate),
                _ => {}
            }
        }
    }

    pub fn handle_click(state: &mut AppState, pos: [f32; 2]) {
        if let Some(tab) = state.current_tab_mut() {
            if tab.modes.active == ModeKind::Annotate {
                tab.modes.annotate.stroke_points.push(pos);
            }
        }
    }
}
