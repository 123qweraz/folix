#[derive(Clone)]
pub struct TextLayout {
    inner: Option<()>,
}

impl TextLayout {
    pub fn empty() -> Self {
        Self { inner: None }
    }

    pub fn is_some(&self) -> bool {
        self.inner.is_some()
    }

    pub fn is_none(&self) -> bool {
        self.inner.is_none()
    }

    pub fn height(&self) -> f32 {
        self.inner.as_ref().map_or(0.0, |_| 0.0)
    }

    pub fn num_chars(&self) -> usize {
        0
    }

    pub fn cursor_left(&self, _char_idx: usize) -> f32 {
        0.0
    }

    pub fn size(&self) -> (f32, f32) {
        (0.0, 0.0)
    }
}
