use std::path::PathBuf;
use std::sync::Arc;

use iced::{keyboard, Task, Point};
use parking_lot::Mutex as PmMutex;

use crate::app::config::AppSettings;
use crate::app::engines::reflow_engine::ReflowDocument;
use crate::app::engines::{ReflowLayout, TextWordPosition};
use crate::app::storage::sqlite::Database;

// ── Tab types ────────────────────────────────────────────────

pub enum TabContent {
    Home,
    Document {
        path: PathBuf,
        document: DocumentHolder,
        current_page: usize,
        scale: f32,
        page_image: Option<iced::widget::image::Handle>,
        word_positions: Vec<TextWordPosition>,
        page_height_pdf: f32,
    },
    Settings,
    PdfToolbox,
}

#[derive(Clone)]
pub enum DocumentHolder {
    Pdf(PdfHolder),
    Reflow(ReflowHolder),
}

#[derive(Clone)]
pub struct PdfHolder {
    pub doc: Arc<SafeDoc>,
    pub page_count: usize,
}

#[derive(Clone)]
pub struct ReflowHolder {
    pub doc: Arc<PmMutex<ReflowDocument>>,
    pub chapter_count: usize,
    pub current_chapter: usize,
    pub chapter_lines: Vec<String>,
}

pub struct SafeDoc(pub PmMutex<mupdf::Document>);
unsafe impl Send for SafeDoc {}
unsafe impl Sync for SafeDoc {}

pub struct Tab {
    pub title: String,
    pub content: TabContent,
    pub path: Option<PathBuf>,
}

// ── Recent files ─────────────────────────────────────────────

pub struct RecentFile {
    pub path: PathBuf,
    pub title: String,
    pub pinned: bool,
}

// ── Application state ────────────────────────────────────────

pub struct State {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub recent_files: Vec<RecentFile>,
    pub status: String,
    pub db: Option<Database>,
    pub settings: AppSettings,
    pub clipboard_text: String,
    pub context_menu: Option<Point>,
}

// ── Messages ─────────────────────────────────────────────────

#[derive(Clone)]
pub enum Message {
    OpenFile,
    FileSelected(Option<PathBuf>),
    PageRendered(usize, Option<iced::widget::image::Handle>),
    CloseTab(usize),
    ActivateTab(usize),
    AddHomeTab,
    AddSettingsTab,
    AddPdfToolboxTab,
    NextPage,
    PrevPage,
    ZoomIn,
    ZoomOut,
    KeyPressed(keyboard::Key, keyboard::Modifiers),
    NavLeft,
    NavRight,
    NavUp,
    NavDown,
    SettingsChanged(AppSettings),
    PdfOperation(String),
    SelectionFinalize(String),
    CopySelection,
    RightClick,
    DismissContextMenu,
}

// ── State methods ────────────────────────────────────────────

impl State {
    pub fn current_tab(&self) -> &Tab {
        &self.tabs[self.active_tab]
    }

    pub fn current_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_tab]
    }
}

// ── Initial boot ─────────────────────────────────────────────

pub fn boot() -> (State, Task<Message>) {
    let db = Database::open("folix.db").ok();
    let recent_files = load_recent_files(&db);

    let tabs = vec![Tab {
        title: "Home".into(),
        content: TabContent::Home,
        path: None,
    }];

    let state = State {
        tabs,
        active_tab: 0,
        recent_files,
        status: "Press Ctrl+O to open a file".into(),
        db,
        settings: AppSettings::default(),
        clipboard_text: String::new(),
        context_menu: None,
    };

    (state, Task::none())
}

fn load_recent_files(db: &Option<Database>) -> Vec<RecentFile> {
    let _ = db;
    vec![]
}

// ── Document loading helpers ─────────────────────────────────

pub fn load_document(path: PathBuf) -> Option<(PathBuf, DocumentHolder)> {
    let lower = path.to_string_lossy().to_lowercase();
    if lower.ends_with(".pdf") {
        match mupdf::Document::open(path.as_path()) {
            Ok(doc) => {
                let page_count = doc.page_count().unwrap_or(0) as usize;
                let holder = DocumentHolder::Pdf(PdfHolder {
                    doc: Arc::new(SafeDoc(PmMutex::new(doc))),
                    page_count,
                });
                Some((path, holder))
            }
            Err(e) => {
                log::error!("Failed to open PDF: {:?}", e);
                None
            }
        }
    } else if lower.ends_with(".epub") || lower.ends_with(".txt") {
        match ReflowDocument::open(path.to_str().unwrap_or("")) {
            Some(doc) => {
                let chapter_count = doc.chapter_count();
                let chapter_lines = load_chapter_texts(&doc, 0);
                let holder = DocumentHolder::Reflow(ReflowHolder {
                    doc: Arc::new(PmMutex::new(doc)),
                    chapter_count,
                    current_chapter: 0,
                    chapter_lines,
                });
                Some((path, holder))
            }
            None => {
                log::error!("Failed to open EPUB/TXT");
                None
            }
        }
    } else {
        None
    }
}

fn load_chapter_texts(doc: &ReflowDocument, idx: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for line in doc.chapter_text(idx).lines() {
        lines.push(line.to_string());
    }
    if lines.is_empty() {
        lines.push("(empty)".into());
    }
    lines
}

pub fn load_chapter_texts_locked(doc: &Arc<PmMutex<ReflowDocument>>, idx: usize) -> Vec<String> {
    let guard = doc.lock();
    load_chapter_texts(&guard, idx)
}

pub fn load_pdf_word_positions(doc: &SafeDoc, page: usize) -> (Vec<TextWordPosition>, f32) {
    use mupdf::TextExtractOptions;
    let guard = doc.0.lock();
    if let Ok(page_obj) = guard.load_page(page as i32) {
        let height = page_obj.bounds().ok().map_or(792.0, |b| b.height());
        let words = page_obj.words(TextExtractOptions::default()).unwrap_or_default();
        let positions: Vec<TextWordPosition> = words
            .into_iter()
            .map(|w| TextWordPosition {
                text: w.text,
                x0: w.bounds.x0,
                y0: w.bounds.y0,
                x1: w.bounds.x1,
                y1: w.bounds.y1,
            })
            .collect();
        (positions, height)
    } else {
        (vec![], 792.0)
    }
}

pub fn tab_title(tab: &Tab) -> &str {
    match &tab.content {
        TabContent::Home => "🏠 Home",
        TabContent::Settings => "⚙ Settings",
        TabContent::PdfToolbox => "🔧 PDF Tools",
        TabContent::Document { .. } => {
            if let Some(p) = &tab.path {
                p.file_stem().and_then(|s| s.to_str()).unwrap_or("Document")
            } else {
                "Document"
            }
        }
    }
}
