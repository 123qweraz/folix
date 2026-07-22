use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Event, Point, Rectangle, Renderer, Size, Theme};

use crate::iced_app::state;

pub struct ReflowCanvas {
    pub lines: Vec<String>,
    pub font_size: f32,
    pub line_height: f32,
}

#[derive(Default)]
pub struct ReflowState {
    pub drag_start_y: Option<f32>,
    pub drag_current_y: Option<f32>,
}

impl canvas::Program<state::Message> for ReflowCanvas {
    type State = ReflowState;

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
                    state.drag_start_y = Some(pos.y);
                    state.drag_current_y = Some(pos.y);
                    return Some(canvas::Action::capture());
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.drag_start_y.is_some() {
                    if let Some(pos) = cursor.position_in(bounds) {
                        state.drag_current_y = Some(pos.y);
                        return Some(canvas::Action::request_redraw());
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let (Some(ys), Some(ye)) = (state.drag_start_y, state.drag_current_y) {
                    let text = compute_selected_lines(&self.lines, ys, ye, self.font_size, self.line_height);
                    state.drag_start_y = None;
                    state.drag_current_y = None;
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
        let lh = self.font_size * self.line_height;
        let text_color = Color::BLACK;

        let (sel_y0, sel_y1) = match (state.drag_start_y, state.drag_current_y) {
            (Some(ys), Some(ye)) => (ys.min(ye), ys.max(ye)),
            _ => (f32::MAX, f32::MIN),
        };

        for (i, line) in self.lines.iter().enumerate() {
            let y = i as f32 * lh;
            let is_selected = y + lh > sel_y0 && y < sel_y1;
            if is_selected {
                frame.fill_rectangle(
                    Point::new(0.0, y),
                    Size::new(bounds.width, lh),
                    Color { r: 0.2, g: 0.4, b: 1.0, a: 0.3 },
                );
            }

            frame.fill_text(canvas::Text {
                content: line.clone(),
                position: Point::new(4.0, y),
                color: text_color,
                size: iced::Pixels(self.font_size),
                ..canvas::Text::default()
            });
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.drag_start_y.is_some() {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::Text
        }
    }
}

fn compute_selected_lines(
    lines: &[String],
    start_y: f32,
    end_y: f32,
    font_size: f32,
    line_height: f32,
) -> String {
    let lh = font_size * line_height;
    let y0 = start_y.min(end_y);
    let y1 = start_y.max(end_y);
    let i0 = (y0 / lh).floor() as usize;
    let i1 = (y1 / lh).floor() as usize;
    let i0 = i0.min(lines.len().saturating_sub(1));
    let i1 = i1.min(lines.len().saturating_sub(1));

    let mut result = String::new();
    for i in i0..=i1 {
        if i > i0 {
            result.push('\n');
        }
        result.push_str(&lines[i]);
    }
    result
}
