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
pub struct LayoutRow {
    pub line_no: usize,
    pub ci: usize,
    pub bi: usize,
    pub it: u8,
    pub text: String,
    pub height: f32,
    /// Character offset of this row within its block (including newlines).
    pub char_offset: usize,
    /// Cached text galley from Fonts::layout_delayed_color (avoid per-frame layout).
    pub galley: Option<std::sync::Arc<egui::Galley>>,
    /// Generation counter: matched against ReadingState::layout_cache_gen to detect staleness.
    pub layout_gen: u64,
    /// Heading level (1-6, 0 = body text).
    pub heading_level: u8,
    pub bold: bool,
    pub italic: bool,
    pub list_item: bool,
    /// Target chapter index for link rows (it=4).
    pub target_ci: Option<usize>,
}

#[derive(Clone)]
pub struct SelectionState {
    pub selecting: bool,
    pub anchor: Option<(f32, f32)>,
    pub focus: Option<(f32, f32)>,
    pub page: usize,
    pub selected_word_indices: Vec<usize>,
    // Character-based selection (for EPUB/TXT plain text): (chapter_idx, block_idx, char_pos)
    pub char_anchor: Option<(usize, usize, usize)>,
    pub char_focus: Option<(usize, usize, usize)>,
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

#[derive(Clone)]
pub struct MagnifierState {
    pub active: bool,
    pub chinese_char: String,
    pub source_line: String,
    pub font_size: f32,
}

impl Default for MagnifierState {
    fn default() -> Self {
        Self { active: false, chinese_char: String::new(), source_line: String::new(), font_size: 64.0 }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum SidebarSection {
    TOC,
    Search,
    Bookmarks,
    Vocab,
    Sentences,
    Magnifier,
}

#[derive(Clone)]
pub struct LayoutState {
    /// Reflow continuous stream: number of pages loaded into the scroll stream.
    pub stream_page_end: usize,
    /// Reflow stream: Y offset of each page's first rendered element.
    pub stream_page_y_starts: Vec<f32>,
    /// Pending jump-to-page request (consumed by renderer).
    pub stream_jump_to: Option<usize>,
    /// Cached chapter data for reflow stream (loaded once, not per-frame).
    pub chapter_cache: Vec<Option<crate::app::engines::Chapter>>,
    /// Velocity-based scroll (px/s). Positive = down, negative = up. 0 = idle.
    pub scroll_velocity: f32,
    /// Max text column width (0 = unlimited).
    pub max_text_width: f32,
    /// Show line numbers for reflow documents.
    pub show_line_numbers: bool,
    /// Layout cache for per-source-line rendering.
    pub layout_cache_rows: Vec<LayoutRow>,
    pub layout_cache_starts: Vec<f32>,
    pub layout_cache_font_size: f32,
    pub layout_cache_avail_w: f32,
    pub layout_cache_line_spacing: f32,
    pub layout_cache_margin_h: f32,
    pub layout_cache_show_ln: bool,
    /// Debounce: set to avail_w when a resize is detected, cleared on rebuild.
    pub layout_cache_pending_avail_w: f32,
    /// Generation counter incremented on every cache rebuild / partial update.
    pub layout_cache_gen: u64,
    /// Current line at the top of the viewport (updated every frame).
    pub current_line: usize,
    /// Total number of source lines in the document.
    pub total_lines: usize,
    /// Input buffer for jump-to-line.
    pub goto_line_text: String,
    /// Pending scroll Y position (set by link clicks, consumed before ScrollArea init).
    pub pending_scroll_y: Option<f32>,
    /// Next chapter index for Phase 2 image-only loading.
    pub next_img_load_ci: usize,
    pub scroll_offset_y: f32,
    pub total_height: f32,
    /// Line number highlighted by 摸鱼 mode (None = no highlight).
    pub mo_yu_playing_line: Option<usize>,
}

#[derive(Clone)]
pub struct VocabState {
    pub show_add_vocab_dialog: bool,
    pub add_vocab_text: String,
    pub vocab: Vec<Vocabulary>,
    pub vocab_dirty: bool,
    pub sentences: Vec<Sentence_>,
    pub sentences_dirty: bool,
}

#[derive(Clone)]
pub struct ReadingState {
    pub view_mode: ViewMode,
    pub show_sidebar: bool,
    pub show_reading_settings: bool,
    pub sidebar_width: f32,
    pub sidebar_section: SidebarSection,
    pub show_goto_dialog: bool,
    pub goto_page_text: String,
    pub search: SearchState,
    pub bookmarks: Vec<Bookmark>,
    pub bookmarks_dirty: bool,
    pub selection: SelectionState,
    pub layout: LayoutState,
    pub vocab_state: VocabState,
    pub magnifier: MagnifierState,
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
    /// Main reader's current_line at activation (reflow) or page (fixed).
    pub main_line: usize,
    /// Global line number of the first line in the current chapter.
    pub base_line: usize,
    /// Current global line number for display.
    pub display_line_no: usize,
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
            main_line: 0,
            base_line: 0,
            display_line_no: 0,
        }
    }
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
            ModeKind::LightReading => "Basic",
            ModeKind::DeepReading => "Annotate",
            ModeKind::PageEdit => "Page Edit",
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

    pub reading: ReadingState,
    pub auto: AutoState,
    pub mo_yu: MoYuState,
    pub edit: EditState,
    pub active: ModeKind,
    pub reflow_font_size: f32,
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
            reading: ReadingState {
                view_mode: ViewMode::Text,
                show_sidebar: false,
                show_reading_settings: false,
                sidebar_width: 260.0,
                sidebar_section: SidebarSection::TOC,
                show_goto_dialog: false,
                goto_page_text: String::new(),
                search: SearchState::new(),
                bookmarks: vec![],
                bookmarks_dirty: false,
                selection: SelectionState::default(),
                layout: LayoutState {
                    stream_page_end: 0,
                    stream_page_y_starts: vec![],
                    stream_jump_to: None,
                    scroll_velocity: 0.0,
                    chapter_cache: vec![],
                    max_text_width: 720.0,
                    show_line_numbers: false,
                    layout_cache_rows: vec![],
                    layout_cache_starts: vec![],
                    layout_cache_font_size: 0.0,
                    layout_cache_avail_w: 0.0,
                    layout_cache_line_spacing: 0.0,
                    layout_cache_margin_h: 0.0,
                    layout_cache_show_ln: false,
                    layout_cache_pending_avail_w: 0.0,
                    layout_cache_gen: 0,
                    current_line: 0,
                    total_lines: 0,
                    goto_line_text: String::new(),
                    pending_scroll_y: None,
                    next_img_load_ci: 0,
                    scroll_offset_y: 0.0,
                    total_height: 0.0,
                    mo_yu_playing_line: None,
                },
                vocab_state: VocabState {
                    show_add_vocab_dialog: false,
                    add_vocab_text: String::new(),
                    vocab: vec![],
                    vocab_dirty: false,
                    sentences: vec![],
                    sentences_dirty: false,
                },
                magnifier: MagnifierState::default(),
            },
            auto: AutoState {
                playing: false,
                speed: 1.0,
                progress: 0.0,
            },
            mo_yu: MoYuState::new(),
            edit: EditState::Page(PageEditState),
            active: ModeKind::LightReading,
            reflow_font_size: 16.0,
        }
    }

    pub fn switch_to(&mut self, target: ModeKind) {
        if self.active == ModeKind::LightReading && target != ModeKind::LightReading {
            self.auto.playing = false;
        }
        self.active = target;
    }
}
