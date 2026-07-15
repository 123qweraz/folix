# Folix Bug & Architecture Issue Report

Generated from code review on 2026-07-15.

---

## 🔴 Bugs

### Bug 1: `bookmarks_dirty` never set on shortcut add bookmark

**File:** `src/app/ui/shell.rs:590-597`

When adding a bookmark via shortcut (`Ctrl+B`), `bookmarks_dirty` is not set to `true`, so the bookmark is never persisted to the database. The sidebar "Add Bookmark" button correctly sets it (`mode_ui.rs:1949`).

### Bug 2: SQLite `save_progress` PRIMARY KEY misuse

**File:** `src/app/storage/sqlite.rs:86-95`

```sql
INSERT INTO progress (id, book_id, page, progress_pct, updated_at)
VALUES (?1, ?2, ?3, ?4, ?5)
ON CONFLICT(id) DO UPDATE SET ...
params![book_id, book_id, page as i64, progress_pct, now]
```

The `id` column (PRIMARY KEY) is set to `book_id` value. This works accidentally (since each book has exactly one progress row), but it's semantically wrong — `id` should be a UUID and `ON CONFLICT` should be on `book_id`.

### Bug 3: Reflow document progress never restored

**File:** `src/app/ui/shell.rs:54-66`

`synchronize_progress` only saves `tab.modes.page` and `auto.progress`. For reflow documents (EPUB/TXT), `page` stays 0 and the actual reading position is in `scroll_offset_y` / `current_line`, which are never saved. Reopening a reflow doc loses reading position.

### Bug 4: Tab key shortcut conflict

**Files:** `src/app/ui/shell.rs:458-462`, `src/app/core/shortcuts.rs:139`

Tab key is manually consumed in `update()` to toggle UI visibility, but there's also a `ToggleUI` shortcut bound to Tab in the shortcut system. The manual consume fires first, so the shortcut system's `ToggleUI` action never triggers.

### Bug 5: Image texture cache cross-tab collision & unbounded growth

**File:** `src/app/ui/mode_ui.rs:654`

```rust
let key = format!("epub_img_{}_{}", rows[i].ci, rows[i].bi);
```

The `image_texture_cache` is a shared `HashMap` in `FolixApp`, but keys only use chapter/block indices. Two tabs with the same document will collide. Also, the cache never evicts entries — unbounded memory growth.

### Bug 6: Unnecessary `render_page` call every frame

**File:** `src/app/ui/mode_ui.rs:121`

```rust
if *page > 0 { fixed.render_page(*page - 1, *scale); }
```

The previous page is rendered every frame regardless of cache state. The returned `Option<RenderedPage>` is discarded; only the side effect (internal caching) is used. This wastes CPU/GPU.

### Bug 7: Cache eviction is FIFO, not LRU

**File:** `src/app/engines/pdf_engine.rs:128-130`

```rust
let oldest = *cache.keys().min().unwrap();
cache.remove(&oldest);
```

Removes the smallest page number (always page 0), not the least recently used entry. If the user is reading near page 100, page 0 keeps getting evicted and reloaded (cache thrashing).

### Bug 8: Text selection character offset is a rough estimate

**File:** `src/app/ui/mode_ui.rs:809-811`

```rust
let approx_char = (ratio * rows[i].text.chars().count() as f32) as usize;
```

For wrapped text, the linear ratio of click position to text width doesn't map correctly to character position. This causes inaccurate text selection in reflow documents.

### Bug 9: `Annotation.doc_id` always empty string

**File:** `src/app/ui/shell.rs:379,1195`, `src/app/ui/mode_ui.rs:1195`

All annotation creation sites pass `doc_id: String::new()`, losing the link to the book.

### Bug 10: Reflow selection not cleared after vocab add

**File:** `src/app/ui/shell.rs:1244,1253`

After adding a word to vocabulary in a reflow document, `selected_word_indices` is cleared but `char_anchor`/`char_focus` are not, leaving the selection highlight visible.

### Bug 11: Config path display incorrect

**File:** `src/app/ui/shell.rs:1160`

Settings page shows "Config file: ./folix.conf" but actual path is `~/.config/folix/folix.conf` (per `config.rs:5-15`).

### Bug 12: Button-hold scroll velocity is frame-rate dependent

**File:** `src/app/ui/shell.rs:1498-1499`

```rust
let dn_btn = ui.button("▼");
if dn_btn.clicked() || dn_btn.is_pointer_button_down_on() {
    tab.modes.reading.scroll_velocity = speed;
}
```

Velocity is set every frame while held but cleared at end of frame (mode_ui.rs:871). This causes jerky frame-rate-dependent scroll instead of smooth continuous scroll.

---

## 🟡 Architecture Issues

### A1. God Struct — `ReadingState` has 40+ fields

**File:** `src/app/core/mode_system.rs:146-201`

Carries layout cache, selection, search, vocabulary, sentences, and UI state — completely unrelated concerns in one struct.

### A2. Monolithic Function — `render_document` is 866 lines

**File:** `src/app/ui/mode_ui.rs:8-874`

Handles fixed layout, reflow layout, layout caching, painting, interaction, auto-play, search, and selection — violates Single Responsibility Principle.

### A3. Delete-and-Reinsert Pattern for data sync

**File:** `src/app/ui/shell.rs:1257-1317`

Annotations, vocabulary, sentences, and bookmarks are all synced by DELETE ALL + INSERT ALL instead of upsert. Not atomic (no transaction), loses `created_at` timestamps, causes write amplification.

### A4. Database connection not in Mutex

**File:** `src/app/storage/sqlite.rs`

`rusqlite::Connection` is `!Send`. Currently only used on the main thread, but blocking calls can freeze the UI and no background thread support.

### A5. Errors silently swallowed

```rust
let _ = db.save_progress(...);
```

All database `Result` values are discarded with `let _ =`. Database failures are invisible.

### A6. `DocumentHandle` missing `Send + Sync` bounds

**File:** `src/app/engines/mod.rs:91-94`

Trait objects lack `Send + Sync` bounds. `PdfDocument` works around this with `unsafe impl Send for SafeDoc`, which is fragile.

### A7. UI and business logic tightly coupled

`mode_ui.rs` mixes rendering, layout, selection logic, and annotation drawing. `shell.rs` has database sync inline in UI methods instead of a service layer.

### A8. Unbounded cache growth

- `image_texture_cache` in `FolixApp` never evicts
- `layout_cache_rows` grows with document length, never trimmed
- MoYu `sentences` accumulate across pages

### A9. Font filename collision risk

**File:** `src/app/ui/shell.rs:106-107`

```rust
let safe = stem.replace(|c: char| !c.is_alphanumeric(), "_");
```

Fonts like "NotoSans-Bold" and "NotoSans Bold" get the same sanitized name and collide in `fonts.font_data`.

### A10. TTC font index hardcoded to 2

**File:** `src/app/ui/shell.rs:111`

```rust
let index = if ext == "ttc" { 2 } else { 0 };
```

Index 2 is unusual for TTC files — should be 0 (first face in collection).
