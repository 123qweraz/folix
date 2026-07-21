use crate::app::engines::{DocumentHandle, reflow_engine::ReflowDocument};
use std::sync::Arc;
use parking_lot::Mutex;

pub struct DocumentManager;

impl DocumentManager {
    pub fn open(path: &str) -> Option<Arc<Mutex<DocumentHandle>>> {
        let lower = path.to_lowercase();
        #[cfg(feature = "egui")]
        if lower.ends_with(".pdf") {
            return crate::app::engines::pdf_engine::PdfDocument::open(path)
                .map(|doc| Arc::new(Mutex::new(DocumentHandle::Fixed(Box::new(doc)))));
        }
        if lower.ends_with(".epub") || lower.ends_with(".txt") || lower.ends_with(".md") || lower.ends_with(".docx") {
            ReflowDocument::open(path).map(|doc| Arc::new(Mutex::new(DocumentHandle::Reflow(Box::new(doc)))))
        } else {
            #[cfg(feature = "egui")]
            if lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg")
                || lower.ends_with(".gif") || lower.ends_with(".bmp") || lower.ends_with(".webp")
                || lower.ends_with(".tiff") || lower.ends_with(".tif") {
                return crate::app::engines::image_engine::ImageDocument::open(path)
                    .map(|doc| Arc::new(Mutex::new(DocumentHandle::Fixed(Box::new(doc)))));
            }
            None
        }
    }
}
