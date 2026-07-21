/// Abstraction over text layout, currently wrapping egui::Galley.
/// Swap the implementation when migrating to iced.

use std::sync::Arc;

#[derive(Clone)]
pub struct TextLayout {
    inner: Option<Arc<egui::Galley>>,
}

impl TextLayout {
    pub fn empty() -> Self {
        Self { inner: None }
    }

    pub fn from_galley(galley: Arc<egui::Galley>) -> Self {
        Self { inner: Some(galley) }
    }

    pub fn as_galley(&self) -> Option<&Arc<egui::Galley>> {
        self.inner.as_ref()
    }

    pub fn take_galley(&mut self) -> Option<Arc<egui::Galley>> {
        self.inner.take()
    }

    pub fn is_some(&self) -> bool {
        self.inner.is_some()
    }

    pub fn is_none(&self) -> bool {
        self.inner.is_none()
    }

    pub fn height(&self) -> f32 {
        self.inner.as_ref().map_or(0.0, |g| g.rect.height().max(1.0))
    }

    pub fn num_chars(&self) -> usize {
        self.inner.as_ref().map_or(0, |g| g.text().len())
    }

    /// Returns the character index closest to (local_x, local_y) within the layout.
    pub fn cursor_at(&self, local_x: f32, local_y: f32) -> usize {
        self.inner.as_ref().map_or(0, |g| {
            g.cursor_from_pos(egui::vec2(local_x.max(0.0), local_y.max(0.0)))
                .ccursor
                .index
        })
    }

    /// Returns the left edge (in local coordinates) of the character at the given index.
    pub fn char_left(&self, char_idx: usize) -> f32 {
        self.inner.as_ref().map_or(0.0, |g| {
            g.pos_from_ccursor(egui::text::CCursor {
                index: char_idx,
                prefer_next_row: false,
            })
            .left()
        })
    }

    pub fn cursor_left(&self, char_idx: usize) -> f32 {
        self.char_left(char_idx)
    }

    pub fn size(&self) -> egui::Vec2 {
        self.inner.as_ref().map_or(egui::Vec2::ZERO, |g| g.rect.size())
    }
}
