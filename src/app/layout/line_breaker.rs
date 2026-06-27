pub struct LineBreaker;

impl LineBreaker {
    pub fn new() -> Self {
        Self
    }

    pub fn break_lines(&self, _text: &str, _max_width: f32) -> Vec<String> {
        vec![]
    }
}
