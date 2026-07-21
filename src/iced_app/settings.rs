use iced::widget::{column, text};
use iced::{Element, Length};

use super::state::{Message, State};

pub fn view(state: &State) -> Element<'_, Message> {
    let s = &state.settings;

    let col = column![
        text("Settings").size(24),
        text("Font Size").size(14),
        text(format!("{:.0}px", s.reading_font_size)).size(12),
        text("Line Height").size(14),
        text(format!("{:.1}", s.reading_line_height)).size(12),
        text("Language").size(14),
        text(&s.language).size(12),
    ]
    .spacing(8)
    .padding(32)
    .width(Length::Fill);

    col.into()
}
