use std::sync::Arc;
use parking_lot::Mutex;
use crate::app::core::document_manager::DocumentManager;
use crate::app::engines;

pub struct ReflowViewerState {
    document: Option<Arc<Mutex<engines::DocumentHandle>>>,
    current_chapter: usize,
}

impl ReflowViewerState {
    pub fn new() -> Self {
        Self { document: None, current_chapter: 0 }
    }

    pub fn open_file(&mut self, path: &str) -> Result<String, String> {
        let handle = DocumentManager::open(path).ok_or_else(|| "Failed to open file".to_string())?;

        if handle.lock().is_reflow() {
            let text = self.load_chapter_text(&handle, 0);
            self.document = Some(handle);
            self.current_chapter = 0;
            Ok(text)
        } else {
            Err("Not a reflow document (PDF)".to_string())
        }
    }

    fn load_chapter_text(&self, doc: &Arc<Mutex<engines::DocumentHandle>>, idx: usize) -> String {
        let guard = doc.lock();
        if let Some(reflow) = guard.as_reflow() {
            reflow.chapter_text(idx)
        } else {
            String::new()
        }
    }

    pub fn current_text(&self) -> String {
        match &self.document {
            Some(doc) => self.load_chapter_text(doc, self.current_chapter),
            None => String::new(),
        }
    }

    pub fn current_title(&self) -> String {
        match &self.document {
            Some(doc) => {
                let guard = doc.lock();
                if let Some(reflow) = guard.as_reflow() {
                    let n = reflow.chapter_count();
                    if self.current_chapter < n {
                        let toc = guard.toc_entries();
                        if self.current_chapter < toc.len() && !toc[self.current_chapter].label.is_empty() {
                            return toc[self.current_chapter].label.clone();
                        }
                    }
                }
                guard.title()
            }
            None => String::new(),
        }
    }

    pub fn has_document(&self) -> bool {
        self.document.is_some()
    }

    pub fn chapter_count(&self) -> usize {
        match &self.document {
            Some(doc) => {
                let guard = doc.lock();
                if let Some(reflow) = guard.as_reflow() {
                    reflow.chapter_count()
                } else {
                    0
                }
            }
            None => 0,
        }
    }

    pub fn current_chapter_index(&self) -> usize {
        self.current_chapter
    }

    pub fn go_to_chapter(&mut self, idx: usize) {
        if let Some(doc) = &self.document {
            let guard = doc.lock();
            if let Some(reflow) = guard.as_reflow() {
                if idx < reflow.chapter_count() {
                    drop(guard);
                    self.current_chapter = idx;
                }
            }
        }
    }

    pub fn prev_chapter(&mut self) {
        if self.current_chapter > 0 {
            self.go_to_chapter(self.current_chapter - 1);
        }
    }

    pub fn next_chapter(&mut self) {
        let max = self.chapter_count();
        if self.current_chapter + 1 < max {
            self.go_to_chapter(self.current_chapter + 1);
        }
    }
}
