use crate::app::core::{AppState, feature_system::Feature};

pub fn render_feature_bar(ui: &mut egui::Ui, state: &mut AppState, features: &[&Feature]) {
    for feature in features {
        let label = format!("[{}] {}", if feature.pinned { "📌" } else { "" }, feature.id);
        if ui.button(&label).clicked() {
            state.feature_system.use_feature(&feature.id);
        }
    }
}
