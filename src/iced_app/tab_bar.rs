use iced::widget::{button, row, text, Row};
use iced::{Element, Length, Theme};

use super::state::{Message, State, tab_title};

pub fn view(state: &State) -> Element<'_, Message> {
    let active_idx = state.active_tab;
    let mut tabs_row: Row<'_, Message> = row![].spacing(0).height(28);

    for (i, tab) in state.tabs.iter().enumerate() {
        let is_active = i == active_idx;
        let title = tab_title(tab).to_string();

        let btn = button(
            row![
                text(title).size(13),
                text(" ×").size(11),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 6])
        .style(move |theme: &Theme, status| {
            if is_active {
                button::primary(theme, status)
            } else {
                button::secondary(theme, status)
            }
        })
        .on_press(Message::ActivateTab(i));

        let close_btn = button(text("×").size(10))
            .padding([0, 2])
            .on_press(Message::CloseTab(i));

        tabs_row = tabs_row.push(
            row![btn, close_btn].spacing(0).align_y(iced::Alignment::Center),
        );
    }

    let add_btn = button(text("+").size(16))
        .padding([2, 8])
        .on_press(Message::AddHomeTab);

    let settings_btn = button(text("⚙").size(14))
        .padding([2, 6])
        .on_press(Message::AddSettingsTab);

    let toolbox_btn = button(text("🔧").size(12))
        .padding([2, 6])
        .on_press(Message::AddPdfToolboxTab);

    tabs_row = tabs_row.push(add_btn).push(settings_btn).push(toolbox_btn);

    tabs_row
        .width(Length::Fill)
        .padding([4, 8])
        .spacing(2)
        .into()
}
