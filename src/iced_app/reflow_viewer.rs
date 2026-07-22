use iced::widget::{button, canvas, column, row, scrollable, text};
use iced::{Element, Length};

use super::reflow_selection::ReflowCanvas;
use super::state::{DocumentHolder, Message, Tab, TabContent};

pub fn view(tab: &Tab) -> Element<'_, Message> {
    match &tab.content {
        TabContent::Document {
            document:
                DocumentHolder::Reflow(holder),
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

            let font_size = 16.0;
            let line_height = 1.4;
            let total_h = holder.chapter_lines.len() as f32 * font_size * line_height + 8.0;

            let canvas_widget = canvas::Canvas::new(ReflowCanvas {
                lines: holder.chapter_lines.clone(),
                font_size,
                line_height,
            })
            .width(Length::Fill)
            .height(Length::Fixed(total_h));

            let scroll = scrollable(canvas_widget)
                .width(Length::Fill)
                .height(Length::Fill);

            column![header, nav, scroll,].spacing(8).padding(8).into()
        }
        _ => text("Not a reflow document").into(),
    }
}
