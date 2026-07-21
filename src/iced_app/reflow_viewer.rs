use iced::widget::{button, column, row, scrollable, text};
use iced::{Element, Length};

use super::state::{DocumentHolder, Message, Tab, TabContent};

pub fn view(tab: &Tab) -> Element<'_, Message> {
    match &tab.content {
        TabContent::Document {
            document: DocumentHolder::Reflow(holder),
            ..
        } => {
            let header = text(format!(
                "Chapter {}/{}",
                holder.current_chapter + 1,
                holder.chapter_count,
            ))
            .size(14);

            let nav = row![
                button("◀").on_press(Message::PrevPage),
                text(format!(" {} ", holder.current_chapter + 1)).size(14),
                button("▶").on_press(Message::NextPage),
            ]
            .spacing(4);

            let mut body = column![].spacing(2).padding(16);
            for line in &holder.chapter_lines {
                body = body.push(text(line).size(16));
            }

            let scroll = scrollable(body)
                .width(Length::Fill)
                .height(Length::Fill);

            column![header, nav, scroll,].spacing(8).padding(8).into()
        }
        _ => text("Not a reflow document").into(),
    }
}
