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
            let full_text = holder.chapter_lines.join("\n");
            let total_chars = full_text.len();
            let est_lines = (total_chars as f32 / 55.0).ceil() as usize + holder.chapter_lines.len();
            let total_h = est_lines as f32 * font_size * line_height + 8.0;

            let canvas_widget = canvas::Canvas::new(ReflowCanvas {
                text: full_text,
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
