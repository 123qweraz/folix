use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub enum ShortcutAction {
    OpenFile,
    CloseTab,
    Quit,
    Reload,
    ZoomIn,
    ZoomOut,
    FitPage,
    ActualSize,
    FitWidth,
    NextPage,
    PrevPage,
    FirstPage,
    LastPage,
    ScrollDown,
    ScrollUp,
    HighlightSel,
    AddBookmark,
    ToggleSidebar,
    ToggleUI,
    Copy,
}

impl ShortcutAction {
    pub fn label(&self) -> &str {
        match self {
            ShortcutAction::OpenFile => "Open File",
            ShortcutAction::CloseTab => "Close Tab",
            ShortcutAction::Quit => "Quit",
            ShortcutAction::Reload => "Reload",
            ShortcutAction::ZoomIn => "Zoom In",
            ShortcutAction::ZoomOut => "Zoom Out",
            ShortcutAction::FitPage => "Fit Page",
            ShortcutAction::ActualSize => "Actual Size",
            ShortcutAction::FitWidth => "Fit Width",
            ShortcutAction::NextPage => "Next Page",
            ShortcutAction::PrevPage => "Prev Page",
            ShortcutAction::FirstPage => "First Page",
            ShortcutAction::LastPage => "Last Page",
            ShortcutAction::ScrollDown => "Scroll Down",
            ShortcutAction::ScrollUp => "Scroll Up",
            ShortcutAction::HighlightSel => "Highlight Selection",
            ShortcutAction::AddBookmark => "Add Bookmark",
            ShortcutAction::ToggleSidebar => "Toggle Sidebar",
            ShortcutAction::ToggleUI => "Toggle UI",
            ShortcutAction::Copy => "Copy",
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyCombo {
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl KeyCombo {
    pub fn check(&self, ctx: &egui::Context) -> bool {
        if let Some(ekey) = key_from_str(&self.key) {
            let mods = self.to_mods();
            ctx.input_mut(|i| i.consume_key(mods, ekey))
        } else {
            false
        }
    }

    pub fn to_mods(&self) -> egui::Modifiers {
        egui::Modifiers {
            alt: self.alt,
            ctrl: self.ctrl,
            shift: self.shift,
            mac_cmd: false,
            command: self.ctrl,
        }
    }

    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl { parts.push("Ctrl"); }
        if self.alt { parts.push("Alt"); }
        if self.shift { parts.push("Shift"); }
        let key_label = match self.key.as_str() {
            "Space" => "Space".to_string(),
            "ArrowLeft" => "←".to_string(),
            "ArrowRight" => "→".to_string(),
            "ArrowUp" => "↑".to_string(),
            "ArrowDown" => "↓".to_string(),
            "PageUp" => "PgUp".to_string(),
            "PageDown" => "PgDn".to_string(),
            "Home" => "Home".to_string(),
            "End" => "End".to_string(),
            "Tab" => "Tab".to_string(),
            k if k.starts_with('F') => k.to_string(),
            k => k.to_string(),
        };
        parts.push(&key_label);
        parts.join("+")
    }
}

pub type ShortcutMap = std::collections::HashMap<ShortcutAction, KeyCombo>;

pub fn default_shortcuts() -> ShortcutMap {
    let mut m = std::collections::HashMap::new();
    m.insert(ShortcutAction::OpenFile, KeyCombo { key: "O".into(), ctrl: true, shift: false, alt: false });
    m.insert(ShortcutAction::CloseTab, KeyCombo { key: "W".into(), ctrl: true, shift: false, alt: false });
    m.insert(ShortcutAction::Quit, KeyCombo { key: "Q".into(), ctrl: true, shift: false, alt: false });
    m.insert(ShortcutAction::Reload, KeyCombo { key: "F5".into(), ctrl: false, shift: false, alt: false });
    m.insert(ShortcutAction::ZoomIn, KeyCombo { key: "Equals".into(), ctrl: true, shift: false, alt: false });
    m.insert(ShortcutAction::ZoomOut, KeyCombo { key: "Minus".into(), ctrl: true, shift: false, alt: false });
    m.insert(ShortcutAction::FitPage, KeyCombo { key: "Num0".into(), ctrl: true, shift: false, alt: false });
    m.insert(ShortcutAction::ActualSize, KeyCombo { key: "Num1".into(), ctrl: true, shift: false, alt: false });
    m.insert(ShortcutAction::FitWidth, KeyCombo { key: "Num2".into(), ctrl: true, shift: false, alt: false });
    m.insert(ShortcutAction::NextPage, KeyCombo { key: "ArrowRight".into(), ctrl: false, shift: false, alt: false });
    m.insert(ShortcutAction::PrevPage, KeyCombo { key: "ArrowLeft".into(), ctrl: false, shift: false, alt: false });
    m.insert(ShortcutAction::FirstPage, KeyCombo { key: "Home".into(), ctrl: false, shift: false, alt: false });
    m.insert(ShortcutAction::LastPage, KeyCombo { key: "End".into(), ctrl: false, shift: false, alt: false });
    m.insert(ShortcutAction::ScrollDown, KeyCombo { key: "Space".into(), ctrl: false, shift: false, alt: false });
    m.insert(ShortcutAction::ScrollUp, KeyCombo { key: "Space".into(), ctrl: false, shift: true, alt: false });
    m.insert(ShortcutAction::HighlightSel, KeyCombo { key: "A".into(), ctrl: false, shift: false, alt: false });
    m.insert(ShortcutAction::AddBookmark, KeyCombo { key: "B".into(), ctrl: true, shift: false, alt: false });
    m.insert(ShortcutAction::ToggleSidebar, KeyCombo { key: "F12".into(), ctrl: false, shift: false, alt: false });
    m.insert(ShortcutAction::ToggleUI, KeyCombo { key: "Tab".into(), ctrl: false, shift: false, alt: false });
    m.insert(ShortcutAction::Copy, KeyCombo { key: "C".into(), ctrl: true, shift: false, alt: false });
    m
}

pub const ALL_ACTIONS: &[ShortcutAction] = &[
    ShortcutAction::OpenFile,
    ShortcutAction::CloseTab,
    ShortcutAction::Quit,
    ShortcutAction::Reload,
    ShortcutAction::ZoomIn,
    ShortcutAction::ZoomOut,
    ShortcutAction::FitPage,
    ShortcutAction::ActualSize,
    ShortcutAction::FitWidth,
    ShortcutAction::NextPage,
    ShortcutAction::PrevPage,
    ShortcutAction::FirstPage,
    ShortcutAction::LastPage,
    ShortcutAction::ScrollDown,
    ShortcutAction::ScrollUp,
    ShortcutAction::HighlightSel,
    ShortcutAction::AddBookmark,
    ShortcutAction::ToggleSidebar,
    ShortcutAction::ToggleUI,
    ShortcutAction::Copy,
];

pub const AVAILABLE_KEYS: &[&str] = &[
    "A","B","C","D","E","F","G","H","I","J","K","L","M",
    "N","O","P","Q","R","S","T","U","V","W","X","Y","Z",
    "Num0","Num1","Num2","Num3","Num4","Num5","Num6","Num7","Num8","Num9",
    "F1","F2","F3","F4","F5","F6","F7","F8","F9","F10","F11","F12",
    "Space","ArrowLeft","ArrowRight","ArrowUp","ArrowDown",
    "PageUp","PageDown","Home","End","Tab",
    "Minus","Equals",
];

fn key_from_str(name: &str) -> Option<egui::Key> {
    match name {
        "A" => Some(egui::Key::A),
        "B" => Some(egui::Key::B),
        "C" => Some(egui::Key::C),
        "D" => Some(egui::Key::D),
        "E" => Some(egui::Key::E),
        "F" => Some(egui::Key::F),
        "G" => Some(egui::Key::G),
        "H" => Some(egui::Key::H),
        "I" => Some(egui::Key::I),
        "J" => Some(egui::Key::J),
        "K" => Some(egui::Key::K),
        "L" => Some(egui::Key::L),
        "M" => Some(egui::Key::M),
        "N" => Some(egui::Key::N),
        "O" => Some(egui::Key::O),
        "P" => Some(egui::Key::P),
        "Q" => Some(egui::Key::Q),
        "R" => Some(egui::Key::R),
        "S" => Some(egui::Key::S),
        "T" => Some(egui::Key::T),
        "U" => Some(egui::Key::U),
        "V" => Some(egui::Key::V),
        "W" => Some(egui::Key::W),
        "X" => Some(egui::Key::X),
        "Y" => Some(egui::Key::Y),
        "Z" => Some(egui::Key::Z),
        "Num0" => Some(egui::Key::Num0),
        "Num1" => Some(egui::Key::Num1),
        "Num2" => Some(egui::Key::Num2),
        "Num3" => Some(egui::Key::Num3),
        "Num4" => Some(egui::Key::Num4),
        "Num5" => Some(egui::Key::Num5),
        "Num6" => Some(egui::Key::Num6),
        "Num7" => Some(egui::Key::Num7),
        "Num8" => Some(egui::Key::Num8),
        "Num9" => Some(egui::Key::Num9),
        "F1" => Some(egui::Key::F1),
        "F2" => Some(egui::Key::F2),
        "F3" => Some(egui::Key::F3),
        "F4" => Some(egui::Key::F4),
        "F5" => Some(egui::Key::F5),
        "F6" => Some(egui::Key::F6),
        "F7" => Some(egui::Key::F7),
        "F8" => Some(egui::Key::F8),
        "F9" => Some(egui::Key::F9),
        "F10" => Some(egui::Key::F10),
        "F11" => Some(egui::Key::F11),
        "F12" => Some(egui::Key::F12),
        "Space" => Some(egui::Key::Space),
        "ArrowLeft" => Some(egui::Key::ArrowLeft),
        "ArrowRight" => Some(egui::Key::ArrowRight),
        "ArrowUp" => Some(egui::Key::ArrowUp),
        "ArrowDown" => Some(egui::Key::ArrowDown),
        "PageUp" => Some(egui::Key::PageUp),
        "PageDown" => Some(egui::Key::PageDown),
        "Home" => Some(egui::Key::Home),
        "End" => Some(egui::Key::End),
        "Tab" => Some(egui::Key::Tab),
        "Minus" => Some(egui::Key::Minus),
        "Equals" => Some(egui::Key::Equals),
        _ => None,
    }
}
