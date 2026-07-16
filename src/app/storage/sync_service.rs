use super::sqlite::Database;
use crate::app::core::app_state::OpenTab;
use crate::app::core::mode_system::{Bookmark, Sentence_, Vocabulary};

pub fn sync_tab(db: &Database, tab: &mut OpenTab) {
    sync_progress(db, &*tab);
    sync_dirty(db, tab);
}

pub fn sync_progress(db: &Database, tab: &OpenTab) {
    if let Some(ref book_id) = tab.book_id {
        if tab.modes.reading.layout.stream_jump_to.is_some() {
            return;
        }
        let is_fixed = tab.document.as_ref().map(|d| d.lock().is_fixed()).unwrap_or(true);
        let page = if is_fixed {
            tab.modes.page
        } else {
            tab.modes.reading.layout.current_line
        };
        if let Err(e) = db.save_progress(book_id, page, tab.modes.auto.progress as f64) {
            eprintln!("[warn] save_progress failed: {}", e);
        }
    }
}

pub fn sync_dirty(db: &Database, tab: &mut OpenTab) {
    if let Some(ref book_id) = tab.book_id {
        sync_vocabulary(db, book_id, &mut tab.modes.reading.vocab_state.vocab, &mut tab.modes.reading.vocab_state.vocab_dirty);
        sync_sentences(db, book_id, &mut tab.modes.reading.vocab_state.sentences, &mut tab.modes.reading.vocab_state.sentences_dirty);
        sync_bookmarks(db, book_id, &mut tab.modes.reading.bookmarks, &mut tab.modes.reading.bookmarks_dirty);
    }
}

fn sync_vocabulary(db: &Database, book_id: &str, vocab: &[Vocabulary], dirty: &mut bool) {
    if !*dirty { return; }
    if let Err(e) = db.sync_vocabulary(book_id, vocab) {
        eprintln!("[warn] sync_vocabulary failed: {}", e);
    }
    *dirty = false;
}

fn sync_sentences(db: &Database, book_id: &str, sentences: &[Sentence_], dirty: &mut bool) {
    if !*dirty { return; }
    if let Err(e) = db.sync_sentences(book_id, sentences) {
        eprintln!("[warn] sync_sentences failed: {}", e);
    }
    *dirty = false;
}

fn sync_bookmarks(db: &Database, book_id: &str, bookmarks: &[Bookmark], dirty: &mut bool) {
    if !*dirty { return; }
    if let Err(e) = db.sync_bookmarks(book_id, bookmarks) {
        eprintln!("[warn] sync_bookmarks failed: {}", e);
    }
    *dirty = false;
}
