use iced::widget::{button, canvas, column, image, row, scrollable, stack, text};
use iced::{Element, Length};

use super::pdf_selection::SelectionOverlay;
use super::state::{DocumentHolder, Message, Tab, TabContent};

pub fn view(tab: &Tab) -> Element<'_, Message> {
    match &tab.content {
        TabContent::Document {
            document:
                DocumentHolder::Pdf(holder),
            current_page,
            scale,
            page_image,
            word_positions,
            page_height_pdf,
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

            let overlay = canvas::Canvas::new(SelectionOverlay {
                word_positions: word_positions.clone(),
                scale: *scale,
                page_height_pdf: *page_height_pdf,
            })
            .width(Length::Fill)
            .height(Length::Shrink);

            let content = stack![img, overlay];

            let scroll = scrollable(content)
                .width(Length::Fill)
                .height(Length::Fill);

            column![page_info, nav, scroll,].spacing(8).padding(8).into()
        }
        _ => text("Not a PDF document").into(),
    }
}
