pub mod mode_system;
pub mod app_state;
pub mod document_manager;
pub mod feature_system;
pub mod shortcuts;
pub mod pdf_toolbox;

#[cfg(feature = "egui")]
#[path = "text_layout_egui.rs"]
pub mod text_layout;

#[cfg(not(feature = "egui"))]
#[path = "text_layout_core.rs"]
pub mod text_layout;

pub use mode_system::{ModeKind, TabModes, ReadingLayout, MoYuState};
pub use app_state::{AppState, OpenTab, TabContent, AppSettings};
