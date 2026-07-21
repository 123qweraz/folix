use iced::widget::{button, column, image, row, scrollable, text};
use iced::{Element, Length};

use super::state::{DocumentHolder, Message, Tab, TabContent};

pub fn view(tab: &Tab) -> Element<'_, Message> {
    match &tab.content {
        TabContent::Document {
            document: DocumentHolder::Pdf(holder),
            current_page,
            scale,
            page_image,
            ..
        } => {
            let page_info = text(format!(
                "Page {}/{}  Scale: {:.0}%",
                current_page + 1,
                holder.page_count,
                scale * 100.0,
            ))
            .size(14);

            let nav = row![
                button("◀").on_press(Message::PrevPage),
                text(format!(" {} ", current_page + 1)).size(14),
                button("▶").on_press(Message::NextPage),
                button("−").on_press(Message::ZoomOut),
                button("+").on_press(Message::ZoomIn),
            ]
            .spacing(4);

            let img: Element<'_, Message> = if let Some(handle) = page_image {
                image(handle.clone())
                    .width(Length::Fill)
                    .height(Length::Shrink)
                    .into()
            } else {
                text("Rendering...").into()
            };

            let scroll = scrollable(img)
                .width(Length::Fill)
                .height(Length::Fill);

            column![page_info, nav, scroll,].spacing(8).padding(8).into()
        }
        _ => text("Not a PDF document").into(),
    }
}
