use super::sqlite::Database;

pub struct LibraryIndex;

impl LibraryIndex {
    pub fn index_document(_db: &Database, _path: &str) {
        // TODO: add book record, search indexing
    }

    pub fn list_books(_db: &Database) -> Vec<String> {
        vec![]
    }
}
