#[derive(Clone, Copy, PartialEq)]
pub enum ViewMode {
    Text,
    Image,
}

#[derive(Clone, PartialEq)]
pub struct ReadingState {
    pub page: usize,
    pub scale: f32,
    pub view_mode: ViewMode,
    pub show_toc: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AutoPlayMode {
    PageFlow,
    GlyphReveal,
    SentenceStream,
}

#[derive(Clone, PartialEq)]
pub struct AutoState {
    pub playing: bool,
    pub speed: f32,
    pub auto_mode: AutoPlayMode,
    pub progress: f32,
}

#[derive(Clone, PartialEq)]
pub enum AnnotationTool {
    Highlight,
    Pen,
    Note,
    Eraser,
    Select,
}

#[derive(Clone, PartialEq)]
pub struct Annotation {
    pub id: String,
    pub doc_id: String,
    pub kind: AnnotationTool,
    pub page: usize,
    pub rect: [f32; 4],
    pub note: Option<String>,
}

#[derive(Clone, PartialEq)]
pub struct AnnotateState {
    pub tool: AnnotationTool,
    pub annotations: Vec<Annotation>,
    pub stroke_points: Vec<[f32; 2]>,
    pub page: usize,
}

#[derive(Clone, PartialEq)]
pub enum Mode {
    Reading(ReadingState),
    Auto(AutoState),
    Annotate(AnnotateState),
}

impl Mode {
    pub fn reading() -> Self {
        Mode::Reading(ReadingState {
            page: 0,
            scale: 1.0,
            view_mode: ViewMode::Text,
            show_toc: false,
        })
    }

    pub fn auto() -> Self {
        Mode::Auto(AutoState {
            playing: false,
            speed: 1.0,
            auto_mode: AutoPlayMode::PageFlow,
            progress: 0.0,
        })
    }

    pub fn annotate() -> Self {
        Mode::Annotate(AnnotateState {
            tool: AnnotationTool::Highlight,
            annotations: vec![],
            stroke_points: vec![],
            page: 0,
        })
    }

    pub fn name(&self) -> &str {
        match self {
            Mode::Reading(_) => "Reading",
            Mode::Auto(_) => "Auto",
            Mode::Annotate(_) => "Annotate",
        }
    }
}

pub trait ModeController {
    fn switch(&mut self, mode: Mode);
    fn current(&self) -> &Mode;
}
