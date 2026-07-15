use rusqlite::{Connection, Result, params};
use parking_lot::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS books (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                title TEXT NOT NULL,
                format TEXT NOT NULL,
                added_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS progress (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL UNIQUE,
                page INTEGER NOT NULL DEFAULT 0,
                progress_pct REAL NOT NULL DEFAULT 0.0,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS annotations (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL,
                page INTEGER NOT NULL,
                kind TEXT NOT NULL,
                rect_data TEXT,
                note TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS bookmarks (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL,
                page INTEGER NOT NULL,
                label TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS feature_usage (
                feature_id TEXT NOT NULL,
                count INTEGER NOT NULL DEFAULT 0,
                last_used TEXT NOT NULL,
                PRIMARY KEY (feature_id)
            );
            CREATE TABLE IF NOT EXISTS search_index (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL,
                page INTEGER NOT NULL,
                content TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS vocabulary (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL,
                word TEXT NOT NULL,
                context_sentence TEXT,
                definition TEXT,
                page INTEGER NOT NULL DEFAULT 0,
                added_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sentences (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL,
                text TEXT NOT NULL,
                page INTEGER NOT NULL DEFAULT 0,
                added_at TEXT NOT NULL
            );"
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS books (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                title TEXT NOT NULL,
                format TEXT NOT NULL,
                added_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS progress (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL UNIQUE,
                page INTEGER NOT NULL DEFAULT 0,
                progress_pct REAL NOT NULL DEFAULT 0.0,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS annotations (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL,
                page INTEGER NOT NULL,
                kind TEXT NOT NULL,
                rect_data TEXT,
                note TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS bookmarks (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL,
                page INTEGER NOT NULL,
                label TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS feature_usage (
                feature_id TEXT NOT NULL,
                count INTEGER NOT NULL DEFAULT 0,
                last_used TEXT NOT NULL,
                PRIMARY KEY (feature_id)
            );
            CREATE TABLE IF NOT EXISTS search_index (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL,
                page INTEGER NOT NULL,
                content TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS vocabulary (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL,
                word TEXT NOT NULL,
                context_sentence TEXT,
                definition TEXT,
                page INTEGER NOT NULL DEFAULT 0,
                added_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sentences (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL,
                text TEXT NOT NULL,
                page INTEGER NOT NULL DEFAULT 0,
                added_at TEXT NOT NULL
            );"
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn save_progress(&self, book_id: &str, page: usize, progress_pct: f64) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let existing: Result<String> = self.conn.lock().query_row(
            "SELECT id FROM progress WHERE book_id = ?1",
            params![book_id],
            |row| row.get(0),
        );
        match existing {
            Ok(id) => {
                self.conn.lock().execute(
                    "UPDATE progress SET page = ?1, progress_pct = ?2, updated_at = ?3 WHERE id = ?4",
                    params![page as i64, progress_pct, now, id],
                )?;
            }
            Err(_) => {
                let id = uuid::Uuid::new_v4().to_string();
                self.conn.lock().execute(
                    "INSERT INTO progress (id, book_id, page, progress_pct, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, book_id, page as i64, progress_pct, now],
                )?;
            }
        }
        Ok(())
    }

    pub fn load_progress(&self, book_id: &str) -> Result<Option<(usize, f64)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT page, progress_pct FROM progress WHERE book_id = ?1"
        )?;
        let mut rows = stmt.query_map(params![book_id], |row| {
            Ok((row.get::<_, i64>(0)? as usize, row.get::<_, f64>(1)?))
        })?;
        match rows.next() {
            Some(Ok(result)) => Ok(Some(result)),
            _ => Ok(None),
        }
    }

    pub fn add_bookmark(&self, book_id: &str, page: usize, label: Option<&str>) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.lock().execute(
            "INSERT INTO bookmarks (id, book_id, page, label, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![uuid::Uuid::new_v4().to_string(), book_id, page as i64, label, now],
        )?;
        Ok(())
    }

    pub fn add_annotation(&self, book_id: &str, page: usize, kind: &str, rect_data: Option<&str>, note: Option<&str>) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.lock().execute(
            "INSERT INTO annotations (id, book_id, page, kind, rect_data, note, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![uuid::Uuid::new_v4().to_string(), book_id, page as i64, kind, rect_data, note, now],
        )?;
        Ok(())
    }

    pub fn search(&self, book_id: &str, query: &str) -> Result<Vec<(usize, String)>> {
        let like_pattern = format!("%{}%", query);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT page, content FROM search_index WHERE book_id = ?1 AND content LIKE ?2 LIMIT 50"
        )?;
        let results = stmt.query_map(params![book_id, like_pattern], |row| {
            Ok((row.get::<_, i64>(0)? as usize, row.get::<_, String>(1)?))
        })?;
        results.collect()
    }


    pub fn ensure_book(&self, path: &str, title: &str, format: &str) -> Result<String> {
        let existing: Result<String> = self.conn.lock().query_row(
            "SELECT id FROM books WHERE path = ?1",
            params![path],
            |row| row.get(0),
        );
        if let Ok(id) = existing {
            return Ok(id);
        }
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.lock().execute(
            "INSERT INTO books (id, path, title, format, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, path, title, format, now],
        )?;
        Ok(id)
    }

    pub fn get_annotations(&self, book_id: &str) -> Result<Vec<(String, usize, String, Option<String>, Option<String>)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, page, kind, rect_data, note FROM annotations WHERE book_id = ?1",
        )?;
        let rows = stmt.query_map(params![book_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)? as usize,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?;
        rows.collect()
    }

    pub fn delete_book_annotations(&self, book_id: &str) -> Result<()> {
        self.conn.lock().execute(
            "DELETE FROM annotations WHERE book_id = ?1",
            params![book_id],
        )?;
        Ok(())
    }

    // ── Vocabulary CRUD ──

    pub fn add_vocabulary(&self, book_id: &str, word: &str, context_sentence: Option<&str>, definition: Option<&str>, page: usize) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.lock().execute(
            "INSERT INTO vocabulary (id, book_id, word, context_sentence, definition, page, added_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, book_id, word, context_sentence, definition, page as i64, now],
        )?;
        Ok(id)
    }

    pub fn list_vocabulary(&self, book_id: &str) -> Result<Vec<(String, String, Option<String>, Option<String>, usize)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, word, context_sentence, definition, page FROM vocabulary WHERE book_id = ?1 ORDER BY added_at DESC"
        )?;
        let rows = stmt.query_map(params![book_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)? as usize,
            ))
        })?;
        rows.collect()
    }

    pub fn delete_vocabulary(&self, id: &str) -> Result<()> {
        self.conn.lock().execute("DELETE FROM vocabulary WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn delete_book_vocabulary(&self, book_id: &str) -> Result<()> {
        self.conn.lock().execute("DELETE FROM vocabulary WHERE book_id = ?1", params![book_id])?;
        Ok(())
    }

    // ── Sentences CRUD ──

    pub fn add_sentence(&self, book_id: &str, text: &str, page: usize) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.lock().execute(
            "INSERT INTO sentences (id, book_id, text, page, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, book_id, text, page as i64, now],
        )?;
        Ok(id)
    }

    pub fn list_sentences(&self, book_id: &str) -> Result<Vec<(String, String, usize)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, text, page FROM sentences WHERE book_id = ?1 ORDER BY added_at DESC"
        )?;
        let rows = stmt.query_map(params![book_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? as usize,
            ))
        })?;
        rows.collect()
    }

    pub fn delete_sentence(&self, id: &str) -> Result<()> {
        self.conn.lock().execute("DELETE FROM sentences WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn delete_book_sentences(&self, book_id: &str) -> Result<()> {
        self.conn.lock().execute("DELETE FROM sentences WHERE book_id = ?1", params![book_id])?;
        Ok(())
    }

    // ── Transactional batch sync ──

    pub fn sync_annotations(&self, book_id: &str, annotations: &[crate::app::core::mode_system::Annotation]) -> Result<()> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM annotations WHERE book_id = ?1", params![book_id])?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = tx.prepare(
            "INSERT INTO annotations (id, book_id, page, kind, rect_data, note, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
        )?;
        for ann in annotations {
            let kind_str = format!("{:?}", ann.kind);
            let rect_str = serde_json::to_string(&ann.rect).ok();
            stmt.execute(params![
                uuid::Uuid::new_v4().to_string(),
                book_id,
                ann.page as i64,
                kind_str,
                rect_str,
                ann.note.as_deref(),
                now,
            ])?;
        }
        drop(stmt);
        tx.commit()?;
        Ok(())
    }

    pub fn sync_vocabulary(&self, book_id: &str, vocab_list: &[crate::app::core::mode_system::Vocabulary]) -> Result<()> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM vocabulary WHERE book_id = ?1", params![book_id])?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = tx.prepare(
            "INSERT INTO vocabulary (id, book_id, word, context_sentence, definition, page, added_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
        )?;
        for v in vocab_list {
            stmt.execute(params![
                uuid::Uuid::new_v4().to_string(),
                book_id,
                v.word,
                v.context_sentence.as_deref(),
                v.definition.as_deref(),
                v.page as i64,
                now,
            ])?;
        }
        drop(stmt);
        tx.commit()?;
        Ok(())
    }

    pub fn sync_sentences(&self, book_id: &str, sentence_list: &[crate::app::core::mode_system::Sentence_]) -> Result<()> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM sentences WHERE book_id = ?1", params![book_id])?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = tx.prepare(
            "INSERT INTO sentences (id, book_id, text, page, added_at) VALUES (?1, ?2, ?3, ?4, ?5)"
        )?;
        for s in sentence_list {
            stmt.execute(params![
                uuid::Uuid::new_v4().to_string(),
                book_id,
                s.text,
                s.page as i64,
                now,
            ])?;
        }
        drop(stmt);
        tx.commit()?;
        Ok(())
    }

    pub fn sync_bookmarks(&self, book_id: &str, bookmark_list: &[crate::app::core::mode_system::Bookmark]) -> Result<()> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM bookmarks WHERE book_id = ?1", params![book_id])?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = tx.prepare(
            "INSERT INTO bookmarks (id, book_id, page, label, created_at) VALUES (?1, ?2, ?3, ?4, ?5)"
        )?;
        for bm in bookmark_list {
            stmt.execute(params![
                uuid::Uuid::new_v4().to_string(),
                book_id,
                bm.page as i64,
                bm.label,
                now,
            ])?;
        }
        drop(stmt);
        tx.commit()?;
        Ok(())
    }

    // ── Bookmarks CRUD (for persistence) ──
    pub fn list_bookmarks(&self, book_id: &str) -> Result<Vec<(String, usize, Option<String>)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, page, label FROM bookmarks WHERE book_id = ?1 ORDER BY page ASC"
        )?;
        let rows = stmt.query_map(params![book_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)? as usize,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        rows.collect()
    }

    pub fn delete_bookmark(&self, id: &str) -> Result<()> {
        self.conn.lock().execute("DELETE FROM bookmarks WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn delete_book_bookmarks(&self, book_id: &str) -> Result<()> {
        self.conn.lock().execute("DELETE FROM bookmarks WHERE book_id = ?1", params![book_id])?;
        Ok(())
    }
}
