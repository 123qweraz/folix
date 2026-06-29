use crate::app::core::{AppState, ModeKind};

pub struct InputRouter;

impl InputRouter {
    pub fn handle_key(state: &mut AppState, key: &str) {
        if let Some(tab) = state.current_tab_mut() {
            match key {
                "1" | "!" => tab.modes.switch_to(ModeKind::LightReading),
                "2" | "@" => tab.modes.switch_to(ModeKind::DeepReading),
                "3" | "#" => tab.modes.switch_to(ModeKind::Edit),
                _ => {}
            }
        }
    }

    pub fn handle_click(state: &mut AppState, pos: [f32; 2]) {
        if let Some(tab) = state.current_tab_mut() {
            if tab.modes.active == ModeKind::DeepReading {
                tab.modes.annotate.stroke_points.push(pos);
            }
        }
    }
}
