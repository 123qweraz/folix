use std::sync::{LazyLock, Mutex};

use cosmic_text::{Attrs, Buffer, Cursor, FontSystem, Metrics, Shaping};
use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Event, Point, Rectangle, Renderer, Size, Theme};

use crate::iced_app::state;

static FONT_SYSTEM: LazyLock<Mutex<FontSystem>> = LazyLock::new(|| Mutex::new(FontSystem::new()));

fn make_buffer(
    font_system: &mut FontSystem,
    text: &str,
    font_size: f32,
    line_height: f32,
    width: f32,
) -> Buffer {
    use cosmic_text::Align;
    let metrics = Metrics::new(font_size, font_size * line_height);
    let mut buf = Buffer::new(font_system, metrics);
    buf.set_text(font_system, text, &Attrs::new(), Shaping::Advanced, Some(Align::Left));
    buf.set_size(font_system, Some(width.max(1.0)), None);
    buf
}

fn full_offset(text: &str, cursor: Cursor) -> usize {
    let mut off = 0;
    for (i, line) in text.lines().enumerate() {
        if i == cursor.line {
            return off + cursor.index;
        }
        off += line.len() + 1;
    }
    off
}

pub struct ReflowCanvas {
    pub text: String,
    pub font_size: f32,
    pub line_height: f32,
}

#[derive(Default, Debug, Clone)]
pub struct ReflowState {
    pub drag_start: Option<Cursor>,
    pub drag_current: Option<Cursor>,
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
        if self.text.is_empty() || bounds.size().width == 0.0 || bounds.size().height == 0.0 {
            return None;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let mut fs = FONT_SYSTEM.lock().unwrap();
                    let buf = make_buffer(
                        &mut *fs,
                        &self.text,
                        self.font_size,
                        self.line_height,
                        bounds.size().width,
                    );
                    if let Some(hit) = buf.hit(pos.x, pos.y) {
                        state.drag_start = Some(hit);
                        state.drag_current = Some(hit);
                        return Some(canvas::Action::capture());
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.drag_start.is_some() {
                    if let Some(pos) = cursor.position_in(bounds) {
                        let mut fs = FONT_SYSTEM.lock().unwrap();
                        let buf = make_buffer(
                            &mut *fs,
                            &self.text,
                            self.font_size,
                            self.line_height,
                            bounds.size().width,
                        );
                        if let Some(hit) = buf.hit(pos.x, pos.y) {
                            state.drag_current = Some(hit);
                            return Some(canvas::Action::request_redraw());
                        }
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let (Some(start), Some(current)) = (state.drag_start, state.drag_current) {
                    let a = full_offset(&self.text, start);
                    let b = full_offset(&self.text, current);
                    let (lo, hi) = (a.min(b), a.max(b));
                    let selected = if lo < hi && hi <= self.text.len() {
                        self.text[lo..hi].to_string()
                    } else {
                        String::new()
                    };

                    state.drag_start = None;
                    state.drag_current = None;
                    return Some(canvas::Action::publish(state::Message::SelectionFinalize(
                        selected,
                    )));
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
        if self.text.is_empty() || bounds.size().width == 0.0 || bounds.size().height == 0.0 {
            return vec![canvas::Frame::new(renderer, bounds.size()).into_geometry()];
        }

        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let mut fs = FONT_SYSTEM.lock().unwrap();
        let buffer = make_buffer(
            &mut *fs,
            &self.text,
            self.font_size,
            self.line_height,
            bounds.size().width,
        );

        for run in buffer.layout_runs() {
            // selection highlight
            if let (Some(ds), Some(dc)) = (state.drag_start, state.drag_current) {
                let (sel_start, sel_end) = (ds.min(dc), ds.max(dc));
                if let Some((x_left, x_width)) = run.highlight(sel_start, sel_end) {
                    frame.fill_rectangle(
                        Point::new(x_left, run.line_top),
                        Size::new(x_width, run.line_height),
                        Color {
                            r: 0.2,
                            g: 0.4,
                            b: 1.0,
                            a: 0.5,
                        },
                    );
                }
            }

            // text rendering
            if let (Some(first), Some(last)) = (run.glyphs.first(), run.glyphs.last()) {
                let (start, end) = if first.start <= last.end {
                    (first.start, last.end)
                } else {
                    (last.start, first.end)
                };
                let run_text = &run.text[start..end];

                frame.fill_text(canvas::Text {
                    content: run_text.to_string(),
                    position: Point::new(0.0, run.line_top),
                    color: Color::BLACK,
                    size: iced::Pixels(self.font_size),
                    ..canvas::Text::default()
                });
            }
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
            mouse::Interaction::Text
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::Align;

    #[test]
    fn test_full_offset_basic() {
        assert_eq!(full_offset("abc\ndef", Cursor::new(0, 0)), 0);
        assert_eq!(full_offset("abc\ndef", Cursor::new(0, 3)), 3);
        assert_eq!(full_offset("abc\ndef", Cursor::new(1, 0)), 4);
        assert_eq!(full_offset("abc\ndef", Cursor::new(1, 3)), 7);
    }

    #[test]
    fn test_cosmic_text_hit() {
        let mut fs = FontSystem::new();
        let text = "Hello World\nSecond line";
        let m = Metrics::new(16.0, 16.0 * 1.4);
        let mut b = Buffer::new(&mut fs, m);
        b.set_text(&mut fs, text, &Attrs::new(), Shaping::Advanced, None);
        b.set_size(&mut fs, Some(400.0), None);
        let runs: Vec<_> = b.layout_runs().collect();
        assert!(!runs.is_empty());
        if let Some(run) = runs.first() {
            assert!(b.hit(10.0, run.line_top + run.line_height / 2.0).is_some());
        }
    }

    #[test]
    fn test_global_font_system() {
        let mut fs = FONT_SYSTEM.lock().unwrap();
        let text = "Hello";
        let m = Metrics::new(16.0, 16.0 * 1.4);
        let mut b = Buffer::new(&mut *fs, m);
        b.set_text(&mut *fs, text, &Attrs::new(), Shaping::Advanced, None);
        b.set_size(&mut *fs, Some(400.0), None);
        let runs: Vec<_> = b.layout_runs().collect();
        assert!(!runs.is_empty());
        drop(fs);
    }
}
