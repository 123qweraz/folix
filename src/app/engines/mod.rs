pub mod pdf_engine;
pub mod reflow_engine;

#[derive(Clone)]
pub struct RenderedPage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct TocEntry {
    pub label: String,
    pub page_index: usize,
}

pub trait Document: Send + Sync {
    fn page_count(&self) -> usize;
    fn page_text(&self, page: usize) -> String;
    fn title(&self) -> String;
    fn metadata(&self, key: &str) -> Option<String>;

    /// Render a page as RGBA image. Returns None if not supported (e.g. EPUB/TXT).
    fn render_page(&self, page: usize, scale: f32) -> Option<RenderedPage>;

    /// Page dimensions at given scale. Returns None if unknown.
    fn page_size(&self, page: usize, scale: f32) -> Option<(f32, f32)> {
        self.render_page(page, scale).map(|p| (p.width as f32, p.height as f32))
    }

    /// Whether this document type supports image rendering (i.e. PDF).
    fn supports_image(&self) -> bool { false }

    /// Table of contents. Each entry maps a label to a page index.
    fn toc_entries(&self) -> Vec<TocEntry> {
        vec![]
    }
}
