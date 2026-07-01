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
        }
    }
}

#[derive(Clone)]
pub struct ReadingState {
    pub view_mode: ViewMode,
    pub show_sidebar: bool,
    pub search: SearchState,
    pub bookmarks: Vec<Bookmark>,
    pub scroll_offset_y: f32,
    pub total_height: f32,
    pub selection: SelectionState,
    /// Reflow continuous stream: number of pages loaded into the scroll stream.
    pub stream_page_end: usize,
    /// Reflow stream: Y offset of each page's first rendered element.
    pub stream_page_y_starts: Vec<f32>,
    /// Pending jump-to-page request (consumed by renderer).
    pub stream_jump_to: Option<usize>,
    /// Velocity for continuous scroll (px/s). Positive = down, negative = up.
    pub scroll_velocity: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AutoPlayMode {
    PageFlow,
    GlyphReveal,
    SentenceStream,
}

#[derive(Clone)]
pub struct AutoState {
    pub playing: bool,
    pub speed: f32,
    pub auto_mode: AutoPlayMode,
    pub progress: f32,
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
    pub fn name(&self) -> &str {
        match self {
            ModeKind::LightReading => "Light",
            ModeKind::DeepReading => "Deep",
            ModeKind::PageEdit => "Page",
            ModeKind::ContentEdit => "Content",
        }
    }
}

#[derive(Clone)]
pub struct TabModes {
    pub page: usize,
    pub scale: f32,
    pub reading_layout: ReadingLayout,
    pub paginator: Option<Paginator>,
    pub reading: ReadingState,
    pub auto: AutoState,
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
            paginator: None,
            reading: ReadingState {
                view_mode: ViewMode::Text,
                show_sidebar: false,
                search: SearchState::new(),
                bookmarks: vec![],
                scroll_offset_y: 0.0,
                total_height: 0.0,
                selection: SelectionState::default(),
                stream_page_end: 0,
                stream_page_y_starts: vec![],
                stream_jump_to: None,
                scroll_velocity: 0.0,
            },
            auto: AutoState {
                playing: false,
                speed: 1.0,
                auto_mode: AutoPlayMode::PageFlow,
                progress: 0.0,
            },
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
