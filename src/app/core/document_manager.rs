use crate::app::engines::{DocumentHandle, pdf_engine::PdfDocument, reflow_engine::ReflowDocument};
use std::sync::Arc;
use parking_lot::Mutex;

pub struct DocumentManager;

impl DocumentManager {
    pub fn open(path: &str) -> Option<Arc<Mutex<DocumentHandle>>> {
        let lower = path.to_lowercase();
        if lower.ends_with(".pdf") {
            PdfDocument::open(path)
                .map(|doc| Arc::new(Mutex::new(DocumentHandle::Fixed(Box::new(doc)))))
        } else if lower.ends_with(".epub") || lower.ends_with(".txt") || lower.ends_with(".md") || lower.ends_with(".docx") {
            ReflowDocument::open(path).map(|doc| Arc::new(Mutex::new(DocumentHandle::Reflow(Box::new(doc)))))
        } else {
            None
        }
    }
}
