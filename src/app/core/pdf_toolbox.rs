use std::path::Path;

#[derive(Clone)]
pub struct TocChapter {
    pub title: String,
    pub page: usize,
}

#[derive(Clone, Copy, PartialEq)]
pub enum PdfOperation {
    Merge,
    Split,
    ExtractImages,
    ExtractText,
    ImageToPdf,
}

impl PdfOperation {
    pub fn name(&self) -> &str {
        match self {
            PdfOperation::Merge => "Merge PDFs",
            PdfOperation::Split => "Split PDF",
            PdfOperation::ExtractImages => "Extract Images",
            PdfOperation::ExtractText => "Extract Text",
            PdfOperation::ImageToPdf => "Image(s) → PDF",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum SplitMode {
    Range,
    EveryNPages,
    ByToc,
}

#[derive(Clone)]
pub struct LogEntry {
    pub message: String,
    pub is_error: bool,
}

#[derive(Clone)]
pub struct PdfToolboxState {
    pub input_files: Vec<String>,
    pub operation: PdfOperation,
    pub split_mode: SplitMode,
    pub split_start: usize,
    pub split_end: usize,
    pub split_every_n: usize,
    pub output_dir: Option<String>,
    pub log: Vec<LogEntry>,
    pub running: bool,
    pub toc_chapters: Vec<TocChapter>,
    pub merge_before_after_page: usize,
}

impl PdfToolboxState {
    pub fn new() -> Self {
        Self {
            input_files: Vec::new(),
            operation: PdfOperation::Merge,
            split_mode: SplitMode::Range,
            split_start: 1,
            split_end: 1,
            split_every_n: 10,
            output_dir: None,
            log: Vec::new(),
            running: false,
            toc_chapters: Vec::new(),
            merge_before_after_page: 1,
        }
    }

    pub fn can_execute(&self) -> bool {
        if self.running {
            return false;
        }
        match self.operation {
            PdfOperation::Merge => self.input_files.len() >= 2,
            PdfOperation::Split => !self.input_files.is_empty(),
            PdfOperation::ExtractImages => !self.input_files.is_empty(),
            PdfOperation::ExtractText => !self.input_files.is_empty(),
            PdfOperation::ImageToPdf => !self.input_files.is_empty(),
        }
    }

    pub fn default_output_name(&self) -> String {
        if self.input_files.is_empty() {
            return "output".into();
        }
        match self.operation {
            PdfOperation::Merge => {
                let stem = Path::new(&self.input_files[0])
                    .file_stem().and_then(|s| s.to_str()).unwrap_or("merged");
                format!("{}_merged.pdf", stem)
            }
            PdfOperation::Split => {
                let stem = Path::new(&self.input_files[0])
                    .file_stem().and_then(|s| s.to_str()).unwrap_or("split");
                format!("{}_split_p{}-p{}.pdf", stem, self.split_start, self.split_end)
            }
            PdfOperation::ExtractImages => {
                let stem = Path::new(&self.input_files[0])
                    .file_stem().and_then(|s| s.to_str()).unwrap_or("extracted");
                format!("{}_p{}.png", stem, self.split_start)
            }
            PdfOperation::ExtractText => {
                let stem = Path::new(&self.input_files[0])
                    .file_stem().and_then(|s| s.to_str()).unwrap_or("extracted");
                format!("{}.txt", stem)
            }
            PdfOperation::ImageToPdf => {
                let stem = Path::new(&self.input_files[0])
                    .file_stem().and_then(|s| s.to_str()).unwrap_or("output");
                format!("{}.pdf", stem)
            }
        }
    }

    pub fn resolve_output_dir(&self) -> String {
        if let Some(ref dir) = self.output_dir {
            dir.clone()
        } else if !self.input_files.is_empty() {
            let p = Path::new(&self.input_files[0]);
            p.parent().map(|d| d.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".into())
        } else {
            ".".into()
        }
    }

    pub fn operation_name(&self) -> &str {
        self.operation.name()
    }

    pub fn clear_log(&mut self) {
        self.log.clear();
    }
}
