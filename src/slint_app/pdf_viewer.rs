use std::sync::Arc;
use parking_lot::Mutex;
use slint::Image;
use slint::{SharedPixelBuffer, Rgba8Pixel};
use crate::app::core::document_manager::DocumentManager;
use crate::app::engines;

pub struct PdfViewerState {
    document: Option<Arc<Mutex<engines::DocumentHandle>>>,
    current_page: usize,
    zoom: f32,
    selection_start: Option<usize>,
    selection_end: Option<usize>,
}

impl PdfViewerState {
    pub fn new() -> Self {
        Self { document: None, current_page: 0, zoom: 1.5, selection_start: None, selection_end: None }
    }

    pub fn open_file(&mut self, path: &str) -> Result<(), String> {
        let handle = DocumentManager::open(path).ok_or_else(|| "Failed to open file".to_string())?;

        if handle.lock().is_fixed() {
            self.document = Some(handle);
            self.current_page = 0;
            self.selection_start = None;
            self.selection_end = None;
            Ok(())
        } else {
            Err("Not a PDF document".to_string())
        }
    }

    pub fn has_document(&self) -> bool {
        self.document.is_some()
    }

    pub fn current_page_index(&self) -> usize {
        self.current_page
    }

    pub fn page_count(&self) -> usize {
        match &self.document {
            Some(doc) => {
                let guard = doc.lock();
                if let Some(fixed) = guard.as_fixed() {
                    fixed.page_count()
                } else {
                    0
                }
            }
            None => 0,
        }
    }

    pub fn document_title(&self) -> String {
        match &self.document {
            Some(doc) => doc.lock().title(),
            None => String::new(),
        }
    }

    pub fn render_current_page(&self) -> Option<Image> {
        let doc = self.document.as_ref()?;
        let guard = doc.lock();
        let fixed = guard.as_fixed()?;
        let mut page = fixed.render_page(self.current_page, self.zoom)?;

        if let Some((start, end)) = self.selection_range() {
            let positions = fixed.page_text_positions(self.current_page);
            Self::apply_highlights(&mut page.rgba, page.width, page.height, &positions, start, end, self.zoom);
        }

        let width = page.width as u32;
        let height = page.height as u32;

        let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
        let pixels = buffer.make_mut_slice();
        for (i, chunk) in page.rgba.chunks_exact(4).enumerate() {
            pixels[i] = Rgba8Pixel::new(chunk[0], chunk[1], chunk[2], chunk[3]);
        }

        Some(Image::from_rgba8(buffer))
    }

    fn selection_range(&self) -> Option<(usize, usize)> {
        match (self.selection_start, self.selection_end) {
            (Some(s), Some(e)) => {
                let start = s.min(e);
                let end = s.max(e);
                Some((start, end))
            }
            _ => None,
        }
    }

    fn apply_highlights(
        rgba: &mut [u8],
        width: u32,
        height: u32,
        positions: &[engines::TextWordPosition],
        sel_start: usize,
        sel_end: usize,
        zoom: f32,
    ) {
        for (idx, word) in positions.iter().enumerate() {
            if idx < sel_start || idx > sel_end {
                continue;
            }

            let x0 = (word.x0 * zoom).max(0.0).min(width as f32) as u32;
            let y0 = (word.y0 * zoom).max(0.0).min(height as f32) as u32;
            let x1 = (word.x1 * zoom).max(0.0).min(width as f32) as u32;
            let y1 = (word.y1 * zoom).max(0.0).min(height as f32) as u32;

            if x0 >= x1 || y0 >= y1 {
                continue;
            }

            for y in y0..y1 {
                for x in x0..x1 {
                    let idx = ((y * width + x) * 4) as usize;
                    if idx + 3 < rgba.len() {
                        rgba[idx] = rgba[idx].saturating_add(60).min(255);
                        rgba[idx + 1] = rgba[idx + 1].saturating_sub(80);
                        rgba[idx + 2] = rgba[idx + 2].saturating_sub(80);
                    }
                }
            }
        }
    }

    pub fn handle_click(&mut self, image_x: f32, image_y: f32) {
        let doc = match &self.document {
            Some(d) => d,
            None => return,
        };
        let guard = doc.lock();
        let fixed = match guard.as_fixed() {
            Some(f) => f,
            None => return,
        };

        let pdf_x = image_x / self.zoom;
        let pdf_y = image_y / self.zoom;

        let positions = fixed.page_text_positions(self.current_page);
        let hit = positions.iter().position(|w| {
            pdf_x >= w.x0 && pdf_x <= w.x1 && pdf_y >= w.y0 && pdf_y <= w.y1
        });

        match hit {
            Some(idx) => {
                self.selection_start = Some(idx);
                self.selection_end = Some(idx);
            }
            None => {
                self.selection_start = None;
                self.selection_end = None;
            }
        }
    }

    pub fn has_selection(&self) -> bool {
        self.selection_range().is_some()
    }

    pub fn selected_text(&self) -> String {
        let (start, end) = match self.selection_range() {
            Some(r) => r,
            None => return String::new(),
        };

        let doc = match &self.document {
            Some(d) => d,
            None => return String::new(),
        };
        let guard = doc.lock();
        let fixed = match guard.as_fixed() {
            Some(f) => f,
            None => return String::new(),
        };

        let positions = fixed.page_text_positions(self.current_page);
        positions[start..=end].iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<&str>>()
            .join(" ")
    }

    pub fn go_to_page(&mut self, idx: usize) {
        if let Some(doc) = &self.document {
            let guard = doc.lock();
            if let Some(fixed) = guard.as_fixed() {
                if idx < fixed.page_count() {
                    drop(guard);
                    self.current_page = idx;
                    self.selection_start = None;
                    self.selection_end = None;
                }
            }
        }
    }

    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.go_to_page(self.current_page - 1);
        }
    }

    pub fn next_page(&mut self) {
        let max = self.page_count();
        if self.current_page + 1 < max {
            self.go_to_page(self.current_page + 1);
        }
    }
}
