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

#[derive(Clone)]
pub struct SearchState {
    pub query: String,
    pub show_search: bool,
    pub matches: Vec<usize>,
    pub current_match: usize,
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
        }
    }
}

#[derive(Clone)]
pub struct Bookmark {
    pub page: usize,
    pub label: String,
}

#[derive(Clone)]
pub struct ReadingState {
    pub page: usize,
    pub scale: f32,
    pub view_mode: ViewMode,
    pub reading_layout: ReadingLayout,
    pub show_sidebar: bool,
    pub search: SearchState,
    pub bookmarks: Vec<Bookmark>,
    pub scroll_offset_y: f32,
    pub total_height: f32,
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

#[derive(Clone)]
pub enum AnnotationTool {
    Highlight,
    Pen,
    Note,
    Eraser,
    Select,
}

#[derive(Clone)]
pub struct Annotation {
    pub id: String,
    pub doc_id: String,
    pub kind: AnnotationTool,
    pub page: usize,
    pub rect: [f32; 4],
    pub note: Option<String>,
}

#[derive(Clone)]
pub struct AnnotateState {
    pub tool: AnnotationTool,
    pub annotations: Vec<Annotation>,
    pub stroke_points: Vec<[f32; 2]>,
    pub page: usize,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ModeKind {
    Reading,
    Auto,
    Annotate,
}

impl ModeKind {
    pub fn name(&self) -> &str {
        match self {
            ModeKind::Reading => "Reading",
            ModeKind::Auto => "Auto",
            ModeKind::Annotate => "Annotate",
        }
    }
}

#[derive(Clone)]
pub struct TabModes {
    pub reading: ReadingState,
    pub auto: AutoState,
    pub annotate: AnnotateState,
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
            reading: ReadingState {
                page: 0,
                scale: 1.0,
                view_mode: ViewMode::Text,
                reading_layout: ReadingLayout::Scroll,
                show_sidebar: false,
                search: SearchState::new(),
                bookmarks: vec![],
                scroll_offset_y: 0.0,
                total_height: 0.0,
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
                page: 0,
            },
            active: ModeKind::Reading,
        }
    }

    pub fn switch_to(&mut self, target: ModeKind) {
        let pos = match self.active {
            ModeKind::Reading => self.reading.page as f32,
            ModeKind::Auto => self.auto.progress,
            ModeKind::Annotate => self.annotate.page as f32,
        };
        match target {
            ModeKind::Reading => {
                self.reading.page = pos as usize;
                self.reading.scroll_offset_y = 0.0;
            }
            ModeKind::Auto => self.auto.progress = pos,
            ModeKind::Annotate => self.annotate.page = pos as usize,
        }
        self.active = target;
    }
}
