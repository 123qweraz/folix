pub mod pdf_engine;
pub mod reflow_engine;
pub mod edit_operations;

use egui::{TextureId, TextureHandle};

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

#[derive(Clone, Debug)]
pub struct TextWordPosition {
    pub text: String,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

#[derive(Clone)]
pub struct StoredImage {
    pub raw_bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone)]
pub enum ContentBlock {
    Text(String),
    Image(StoredImage),
}

pub trait Document: Send + Sync {
    fn title(&self) -> String;
    fn toc_entries(&self) -> Vec<TocEntry>;
    fn metadata(&self, key: &str) -> Option<String>;
}

pub trait FixedLayout: Document {
    fn page_count(&self) -> usize;
    fn render_page(&self, page: usize, scale: f32) -> Option<RenderedPage>;
    fn page_size(&self, page: usize, scale: f32) -> Option<(f32, f32)> {
        self.render_page(page, scale).map(|p| (p.width as f32, p.height as f32))
    }
    fn page_text(&self, page: usize) -> String;
    fn page_text_positions(&self, page: usize) -> Vec<TextWordPosition>;
    fn get_texture_handle(&self, page: usize, scale: f32) -> Option<(TextureId, [usize; 2])>;
    fn set_texture_handle(&self, page: usize, scale: f32, handle: TextureHandle);
}

pub trait ReflowLayout: Document {
    fn chapter_count(&self) -> usize;
    fn chapter_text(&self, idx: usize) -> String;
    fn load_chapter(&self, idx: usize) -> Chapter;
}

#[derive(Clone)]
pub struct Chapter {
    pub title: String,
    pub blocks: Vec<ContentBlock>,
}

pub enum DocumentHandle {
    Fixed(Box<dyn FixedLayout>),
    Reflow(Box<dyn ReflowLayout>),
}

impl DocumentHandle {
    pub fn title(&self) -> String {
        match self {
            DocumentHandle::Fixed(d) => d.title(),
            DocumentHandle::Reflow(d) => d.title(),
        }
    }

    pub fn toc_entries(&self) -> Vec<TocEntry> {
        match self {
            DocumentHandle::Fixed(d) => d.toc_entries(),
            DocumentHandle::Reflow(d) => d.toc_entries(),
        }
    }

    pub fn metadata(&self, key: &str) -> Option<String> {
        match self {
            DocumentHandle::Fixed(d) => d.metadata(key),
            DocumentHandle::Reflow(d) => d.metadata(key),
        }
    }

    pub fn is_fixed(&self) -> bool {
        matches!(self, DocumentHandle::Fixed(_))
    }

    pub fn is_reflow(&self) -> bool {
        matches!(self, DocumentHandle::Reflow(_))
    }

    pub fn as_fixed(&self) -> Option<&dyn FixedLayout> {
        match self {
            DocumentHandle::Fixed(d) => Some(&**d),
            DocumentHandle::Reflow(_) => None,
        }
    }

    pub fn as_reflow(&self) -> Option<&dyn ReflowLayout> {
        match self {
            DocumentHandle::Fixed(_) => None,
            DocumentHandle::Reflow(d) => Some(&**d),
        }
    }

    pub fn as_fixed_mut(&mut self) -> Option<&mut dyn FixedLayout> {
        match self {
            DocumentHandle::Fixed(d) => Some(&mut **d),
            DocumentHandle::Reflow(_) => None,
        }
    }

    pub fn as_reflow_mut(&mut self) -> Option<&mut dyn ReflowLayout> {
        match self {
            DocumentHandle::Fixed(_) => None,
            DocumentHandle::Reflow(d) => Some(&mut **d),
        }
    }
}
