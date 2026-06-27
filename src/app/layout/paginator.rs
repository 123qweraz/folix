pub struct Paginator;

impl Paginator {
    pub fn new() -> Self {
        Self
    }

    pub fn paginate(&self, _lines: &[String], _page_height: f32) -> Vec<Vec<String>> {
        vec![]
    }
}
