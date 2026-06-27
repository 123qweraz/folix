use crate::app::engines::{Document, pdf_engine::PdfDocument, reflow_engine::ReflowDocument};
use std::sync::Arc;
use parking_lot::Mutex;

pub struct DocumentManager;

impl DocumentManager {
    pub fn open(path: &str) -> Option<Arc<Mutex<Box<dyn Document>>>> {
        let lower = path.to_lowercase();
        if lower.ends_with(".pdf") {
            PdfDocument::open(path).map(|doc| Arc::new(Mutex::new(Box::new(doc) as Box<dyn Document>)))
        } else if lower.ends_with(".epub") || lower.ends_with(".txt") {
            ReflowDocument::open(path).map(|doc| Arc::new(Mutex::new(Box::new(doc) as Box<dyn Document>)))
        } else {
            None
        }
    }
}
