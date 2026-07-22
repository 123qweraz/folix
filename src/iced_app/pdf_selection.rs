use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Event, Point, Rectangle, Renderer, Size, Theme};

use crate::app::engines::TextWordPosition;
use crate::iced_app::state;

#[derive(Default)]
pub struct PendingSelection {
    pub drag_start: Option<Point>,
    pub drag_current: Option<Point>,
}

pub struct SelectionOverlay {
    pub word_positions: Vec<TextWordPosition>,
    pub scale: f32,
    pub page_height_pdf: f32,
}

impl canvas::Program<state::Message> for SelectionOverlay {
    type State = PendingSelection;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<state::Message>> {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    state.drag_start = Some(pos);
                    state.drag_current = Some(pos);
                    return Some(canvas::Action::capture());
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.drag_start.is_some() {
                    if let Some(pos) = cursor.position_in(bounds) {
                        state.drag_current = Some(pos);
                        return Some(canvas::Action::request_redraw());
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let (Some(start), Some(end)) = (state.drag_start, state.drag_current) {
                    let text = compute_selected_text(
                        &self.word_positions,
                        self.scale,
                        self.page_height_pdf,
                        start,
                        end,
                    );
                    state.drag_start = None;
                    state.drag_current = None;
                    return Some(canvas::Action::publish(state::Message::SelectionFinalize(text)));
                }
            }
            _ => {}
        }
        None
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        if let (Some(start), Some(end)) = (state.drag_start, state.drag_current) {
            let x0 = start.x.min(end.x);
            let y0 = start.y.min(end.y);
            let x1 = start.x.max(end.x);
            let y1 = start.y.max(end.y);

            let sel_rect = Rectangle::new(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0));

            for w in &self.word_positions {
                let sx0 = w.x0 * self.scale;
                let sy0 = (self.page_height_pdf - w.y1) * self.scale;
                let sx1 = w.x1 * self.scale;
                let sy1 = (self.page_height_pdf - w.y0) * self.scale;

                let word_rect = Rectangle::new(Point::new(sx0, sy0), Size::new(sx1 - sx0, sy1 - sy0));
                if rects_overlap(sel_rect, word_rect) {
                    frame.fill_rectangle(
                        Point::new(sx0, sy0),
                        Size::new(sx1 - sx0, sy1 - sy0),
                        Color { r: 0.2, g: 0.4, b: 1.0, a: 0.5 },
                    );
                }
            }

            frame.fill_rectangle(
                Point::new(x0, y0),
                Size::new(x1 - x0, y1 - y0),
                Color { r: 0.2, g: 0.4, b: 1.0, a: 0.15 },
            );
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.drag_start.is_some() {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

fn rects_overlap(a: Rectangle, b: Rectangle) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

fn compute_selected_text(
    words: &[TextWordPosition],
    scale: f32,
    page_height_pdf: f32,
    start: Point,
    end: Point,
) -> String {
    let x0 = start.x.min(end.x);
    let y0 = start.y.min(end.y);
    let x1 = start.x.max(end.x);
    let y1 = start.y.max(end.y);
    let sel = Rectangle::new(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0));

    let mut selected: Vec<(f32, f32, &str)> = Vec::new();
    for w in words {
        let sx0 = w.x0 * scale;
        let sy0 = (page_height_pdf - w.y1) * scale;
        let sx1 = w.x1 * scale;
        let sy1 = (page_height_pdf - w.y0) * scale;
        let r = Rectangle::new(Point::new(sx0, sy0), Size::new(sx1 - sx0, sy1 - sy0));
        if rects_overlap(sel, r) {
            selected.push(((sy0 + sy1) / 2.0, (sx0 + sx1) / 2.0, &w.text));
        }
    }

    selected.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(
        a.1.partial_cmp(&b.1).unwrap(),
    ));

    let mut result = String::new();
    let mut last_y: Option<f32> = None;
    for (cy, _cx, text) in &selected {
        if let Some(ly) = last_y {
            if (cy - ly).abs() > 10.0 {
                result.push('\n');
            } else {
                result.push(' ');
            }
        }
        result.push_str(text);
        last_y = Some(*cy);
    }
    result
}
