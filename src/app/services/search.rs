use crate::app::storage::sqlite::Database;

pub struct SearchService;

impl SearchService {
    pub fn new() -> Self {
        Self
    }

    pub fn search(_db: &Database, _book_id: &str, _query: &str) -> Vec<(usize, String)> {
        vec![]
    }

    pub fn index_page(_db: &Database, _book_id: &str, _page: usize, _content: &str) {
    }
}
