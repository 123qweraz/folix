#[derive(Clone, Copy, PartialEq)]
pub enum ViewMode {
    Text,
    Image,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ReadingLayout {
    Paged,
    Scroll,
}

#[derive(Clone, Copy, PartialEq)]
pub enum FitMode {
    Free,
    FitWidth,
    FitPage,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ViewRotation {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

use std::collections::HashMap;
use crate::app::paginator::Paginator;

#[derive(Clone)]
pub struct SearchState {
    pub query: String,
    pub show_search: bool,
    pub matches: Vec<usize>,
    pub current_match: usize,
    pub page_highlights: HashMap<usize, Vec<usize>>,
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            show_search: false,
            matches: vec![],
            current_match: 0,
            page_highlights: HashMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct Bookmark {
    pub page: usize,
    pub label: String,
}

#[derive(Clone)]
pub struct SelectionState {
    pub selecting: bool,
    pub anchor: Option<(f32, f32)>,
    pub focus: Option<(f32, f32)>,
    pub page: usize,
    pub selected_word_indices: Vec<usize>,
    // Character-based selection (for EPUB/TXT plain text)
    pub char_anchor: Option<usize>,
    pub char_focus: Option<usize>,
    pub selected_text: String,
    /// Pending vocabulary addition (word to add, set by context menu)
    pub pending_vocab: Option<String>,
    /// Pending sentence addition (text to save, set by context menu)
    pub pending_sentence: Option<String>,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            selecting: false,
            anchor: None,
            focus: None,
            page: 0,
            selected_word_indices: vec![],
            char_anchor: None,
            char_focus: None,
            selected_text: String::new(),
            pending_vocab: None,
            pending_sentence: None,
        }
    }
}

#[derive(Clone)]
pub struct Vocabulary {
    pub id: String,
    pub word: String,
    pub context_sentence: Option<String>,
    pub definition: Option<String>,
    pub page: usize,
}

#[derive(Clone)]
pub struct Sentence_ {
    pub id: String,
    pub text: String,
    pub page: usize,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SidebarSection {
    TOC,
    Search,
    Bookmarks,
    Vocab,
    Sentences,
}

#[derive(Clone)]
pub struct ReadingState {
    pub view_mode: ViewMode,
    pub show_sidebar: bool,
    pub sidebar_section: SidebarSection,
    pub show_add_vocab_dialog: bool,
    pub add_vocab_text: String,
    pub show_goto_dialog: bool,
    pub goto_page_text: String,
    pub search: SearchState,
    pub bookmarks: Vec<Bookmark>,
    pub bookmarks_dirty: bool,
    pub scroll_offset_y: f32,
    pub total_height: f32,
    pub selection: SelectionState,
    /// Reflow continuous stream: number of pages loaded into the scroll stream.
    pub stream_page_end: usize,
    /// Reflow stream: Y offset of each page's first rendered element.
    pub stream_page_y_starts: Vec<f32>,
    /// Pending jump-to-page request (consumed by renderer).
    pub stream_jump_to: Option<usize>,
    /// Velocity-based scroll (px/s). Positive = down, negative = up. 0 = idle.
    pub scroll_velocity: f32,
    /// Cached chapter data for reflow stream (loaded once, not per-frame).
    pub chapter_cache: Vec<Option<crate::app::engines::Chapter>>,
    /// Vocabulary (生词本) for the current book.
    pub vocab: Vec<Vocabulary>,
    pub vocab_dirty: bool,
    /// Sentence collection (句子收藏) for the current book.
    pub sentences: Vec<Sentence_>,
    pub sentences_dirty: bool,
}

#[derive(Clone)]
pub struct AutoState {
    pub playing: bool,
    pub speed: f32,
    pub progress: f32,
}

/// 摸鱼模式 — independent sentence-by-sentence floating window
#[derive(Clone)]
pub struct MoYuState {
    pub visible: bool,
    pub playing: bool,
    pub speed: f32,
    pub voice: bool,
    pub sentences: Vec<String>,
    pub sentence_idx: usize,
    pub timer: f32,
    pub page: usize,
    pub scroll_x: f32,
    pub positioned: bool,
}

impl MoYuState {
    pub fn new() -> Self {
        Self {
            visible: false,
            playing: false,
            speed: 1.0,
            voice: false,
            sentences: vec![],
            sentence_idx: 0,
            timer: 0.0,
            page: 0,
            scroll_x: 0.0,
            positioned: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnnotationTool {
    Highlight,
    Pen,
    Note,
    Eraser,
}

#[derive(Clone)]
pub struct Annotation {
    pub id: String,
    pub doc_id: String,
    pub kind: AnnotationTool,
    pub page: usize,
    pub rect: [f32; 4],
    pub note: Option<String>,
    pub color: [u8; 4],
}

pub const HIGHLIGHT_COLORS: [[u8; 4]; 8] = [
    [255, 255, 0, 120],   // yellow
    [255, 150, 50, 120],  // orange
    [255, 100, 100, 120], // red
    [100, 200, 255, 120], // blue
    [100, 255, 100, 120], // green
    [200, 100, 255, 120], // purple
    [255, 255, 255, 120], // white
    [80, 80, 80, 120],    // gray
];

#[derive(Clone)]
pub struct AnnotateState {
    pub tool: AnnotationTool,
    pub annotations: Vec<Annotation>,
    pub stroke_points: Vec<[f32; 2]>,
    pub selecting: bool,
    pub selection_anchor: Option<(f32, f32)>,
    pub selection_focus: Option<(f32, f32)>,
    pub selection_page: usize,
    pub editing_note_id: Option<String>,
    pub note_text_buffer: String,
    pub current_color: [u8; 4],
    pub dirty: bool,
}

#[derive(Clone)]
pub struct PageEditState;

#[derive(Clone)]
pub struct ContentEditState {
    pub font_size_scale: f32,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Clone)]
pub enum EditState {
    Page(PageEditState),
    Content(ContentEditState),
}

impl EditState {
    pub fn as_content(&mut self) -> Option<&mut ContentEditState> {
        match self {
            EditState::Content(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ModeKind {
    LightReading,
    DeepReading,
    PageEdit,
    ContentEdit,
}

impl ModeKind {
    pub fn name(&self, lang: &str) -> &'static str {
        let key = match self {
            ModeKind::LightReading => "Light",
            ModeKind::DeepReading => "Deep",
            ModeKind::PageEdit => "Page",
            ModeKind::ContentEdit => "Content",
        };
        crate::app::i18n::tr(lang, key)
    }
}

#[derive(Clone)]
pub struct TabModes {
    pub page: usize,
    pub scale: f32,
    pub reading_layout: ReadingLayout,
    pub fit_mode: FitMode,
    pub view_rotation: ViewRotation,
    pub paginator: Option<Paginator>,
    pub reading: ReadingState,
    pub auto: AutoState,
    pub mo_yu: MoYuState,
    pub annotate: AnnotateState,
    pub edit: EditState,
    pub active: ModeKind,
}

impl Default for TabModes {
    fn default() -> Self {
        Self::new()
    }
}

impl TabModes {
    pub fn new() -> Self {
        Self {
            page: 0,
            scale: 1.0,
            reading_layout: ReadingLayout::Scroll,
            fit_mode: FitMode::Free,
            view_rotation: ViewRotation::Deg0,
            paginator: None,
            reading: ReadingState {
                view_mode: ViewMode::Text,
                show_sidebar: false,
                sidebar_section: SidebarSection::TOC,
                show_add_vocab_dialog: false,
                add_vocab_text: String::new(),
                show_goto_dialog: false,
                goto_page_text: String::new(),
                search: SearchState::new(),
                bookmarks: vec![],
                bookmarks_dirty: false,
                scroll_offset_y: 0.0,
                total_height: 0.0,
                selection: SelectionState::default(),
                stream_page_end: 0,
                stream_page_y_starts: vec![],
                stream_jump_to: None,
                scroll_velocity: 0.0,
                chapter_cache: vec![],
                vocab: vec![],
                vocab_dirty: false,
                sentences: vec![],
                sentences_dirty: false,
            },
            auto: AutoState {
                playing: false,
                speed: 1.0,
                progress: 0.0,
            },
            mo_yu: MoYuState::new(),
            annotate: AnnotateState {
                tool: AnnotationTool::Highlight,
                annotations: vec![],
                stroke_points: vec![],
                selecting: false,
                selection_anchor: None,
                selection_focus: None,
                selection_page: 0,
                editing_note_id: None,
                note_text_buffer: String::new(),
                current_color: HIGHLIGHT_COLORS[0],
                dirty: false,
            },
            edit: EditState::Page(PageEditState),
            active: ModeKind::LightReading,
        }
    }

    pub fn switch_to(&mut self, target: ModeKind) {
        if self.active == ModeKind::LightReading && target != ModeKind::LightReading {
            self.auto.playing = false;
        }
        self.active = target;
    }
}
