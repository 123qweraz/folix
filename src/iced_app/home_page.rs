use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};

use super::state::{Message, State};

pub fn view(state: &State) -> Element<'_, Message> {
    let mut col = column![]
        .spacing(16)
        .padding(32)
        .width(Length::Fill)
        .align_x(iced::Alignment::Center);

    col = col.push(text("Folix").size(36));
    col = col.push(text(&state.status).size(14));
    col = col.push(
        button(text("📂 Open File...").size(16))
            .padding([8, 24])
            .on_press(Message::OpenFile),
    );

    if !state.recent_files.is_empty() {
        col = col.push(text("Recent Files").size(18));
        let mut recent_col = column![].spacing(4).width(Length::Fixed(400.0));
        for file in &state.recent_files {
            let title = file.title.clone();
            let path_s = file.path.display().to_string();
            recent_col = recent_col.push(
                button(
                    row![
                        text(title).size(14).width(Length::Fill),
                        text(path_s).size(10).color([0.5, 0.5, 0.5]),
                    ]
                    .spacing(8),
                )
                .padding([4, 8])
                .width(Length::Fill)
                .style(button::secondary),
            );
        }
        col = col.push(container(recent_col).width(Length::Fixed(400.0)));
    }

    col.into()
}
