use rusqlite::{Connection, Result, params};

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init_tables()?;
        Ok(db)
    }

    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS books (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                title TEXT NOT NULL,
                format TEXT NOT NULL,
                added_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS progress (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL,
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
            );"
        )?;
        Ok(())
    }

    pub fn save_progress(&self, book_id: &str, page: usize, progress_pct: f64) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO progress (id, book_id, page, progress_pct, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET page=excluded.page, progress_pct=excluded.progress_pct, updated_at=excluded.updated_at",
            params![uuid::Uuid::new_v4().to_string(), book_id, page as i64, progress_pct, now],
        )?;
        Ok(())
    }

    pub fn add_bookmark(&self, book_id: &str, page: usize, label: Option<&str>) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO bookmarks (id, book_id, page, label, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![uuid::Uuid::new_v4().to_string(), book_id, page as i64, label, now],
        )?;
        Ok(())
    }

    pub fn add_annotation(&self, book_id: &str, page: usize, kind: &str, rect_data: Option<&str>, note: Option<&str>) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO annotations (id, book_id, page, kind, rect_data, note, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![uuid::Uuid::new_v4().to_string(), book_id, page as i64, kind, rect_data, note, now],
        )?;
        Ok(())
    }

    pub fn search(&self, book_id: &str, query: &str) -> Result<Vec<(usize, String)>> {
        let like_pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT page, content FROM search_index WHERE book_id = ?1 AND content LIKE ?2 LIMIT 50"
        )?;
        let results = stmt.query_map(params![book_id, like_pattern], |row| {
            Ok((row.get::<_, i64>(0)? as usize, row.get::<_, String>(1)?))
        })?;
        results.collect()
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}
