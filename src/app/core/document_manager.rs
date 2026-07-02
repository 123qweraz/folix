use crate::app::engines::{DocumentHandle, pdf_engine::PdfDocument, reflow_engine::ReflowDocument, image_engine::ImageDocument};
use std::sync::Arc;
use parking_lot::Mutex;

pub struct DocumentManager;

impl DocumentManager {
    pub fn open(path: &str) -> Option<Arc<Mutex<DocumentHandle>>> {
        let lower = path.to_lowercase();
        if lower.ends_with(".pdf") {
            PdfDocument::open(path).map(|doc| Arc::new(Mutex::new(DocumentHandle::Fixed(Box::new(doc)))))
        } else if lower.ends_with(".epub") || lower.ends_with(".txt") || lower.ends_with(".md") || lower.ends_with(".docx") {
            ReflowDocument::open(path).map(|doc| Arc::new(Mutex::new(DocumentHandle::Reflow(Box::new(doc)))))
        } else if lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg")
            || lower.ends_with(".gif") || lower.ends_with(".bmp") || lower.ends_with(".webp")
            || lower.ends_with(".tiff") || lower.ends_with(".tif") {
            ImageDocument::open(path).map(|doc| Arc::new(Mutex::new(DocumentHandle::Fixed(Box::new(doc)))))
        } else {
            None
        }
    }
}
