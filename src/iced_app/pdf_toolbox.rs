use iced::widget::{column, text};
use iced::{Element, Length};

use super::state::{Message, State};

pub fn view(_state: &State) -> Element<'_, Message> {
    column![
        text("PDF Toolbox").size(24),
        text("Coming soon: merge, split, extract...").size(14),
    ]
    .spacing(8)
    .padding(32)
    .width(Length::Fill)
    .into()
}
