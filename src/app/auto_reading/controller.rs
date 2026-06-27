use crate::app::core::mode_system::AutoState;

pub struct AutoController;

impl AutoController {
    pub fn update(state: &mut AutoState, dt: f32) {
        if state.playing {
            state.progress += dt * state.speed * 0.5;
        }
    }
}
