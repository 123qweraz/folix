use super::{Document, RenderedPage, TocEntry};

pub struct ReflowDocument {
    path: String,
    chapters: Vec<String>,
    doc_title: String,
    toc: Vec<TocEntry>,
}

impl ReflowDocument {
    pub fn open(path: &str) -> Option<Self> {
        let lower = path.to_lowercase();
        if lower.ends_with(".epub") {
            Self::open_epub(path)
        } else if lower.ends_with(".txt") {
            Self::open_txt(path)
        } else {
            None
        }
    }

    fn open_epub(path: &str) -> Option<Self> {
        use rbook::Epub;

        let epub = Epub::open(path).ok()?;

        let doc_title = epub
            .metadata()
            .title()
            .map(|t| t.value().to_string())
            .or_else(|| {
                std::path::Path::new(path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "Untitled".to_string());

        // Build chapter content from reader
        let mut chapters: Vec<String> = Vec::new();

        for result in epub.reader() {
            let data = result.ok()?;
            let html = data.content();
            let plain = Self::strip_html(html);
            if !plain.trim().is_empty() {
                chapters.push(plain.trim().to_string());
            }
        }

        if chapters.is_empty() {
            chapters.push("(empty document)".to_string());
        }

        // Build ToC: assign page indices sequentially from flat ToC entries
        let mut toc: Vec<TocEntry> = Vec::new();
        let toc_data = epub.toc();
        if let Some(contents) = toc_data.contents() {
            for (i, entry) in contents.flatten().enumerate() {
                if i < chapters.len() {
                    toc.push(TocEntry {
                        label: entry.label().to_string(),
                        page_index: i,
                    });
                }
            }
        }

        Some(Self {
            path: path.to_string(),
            chapters,
            doc_title,
            toc,
        })
    }

    fn open_txt(path: &str) -> Option<Self> {
        let data = std::fs::read(path).ok()?;
        let content = Self::decode_text(&data);

        let doc_title = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        let chapters = Self::paginate_text(&content, 3000);

        Some(Self {
            path: path.to_string(),
            chapters,
            doc_title,
            toc: vec![],
        })
    }

    fn paginate_text(text: &str, target_chars: usize) -> Vec<String> {
        let paragraphs: Vec<&str> = text.split("\n\n").collect();
        let mut pages = Vec::new();
        let mut current = String::new();

        for para in paragraphs {
            let trimmed = para.trim();
            if trimmed.is_empty() {
                continue;
            }

            if current.len() + trimmed.len() + 2 > target_chars && !current.is_empty() {
                pages.push(std::mem::take(&mut current));
            }

            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(trimmed);
        }

        if !current.is_empty() {
            pages.push(current);
        }

        if pages.is_empty() {
            pages.push("(empty)".to_string());
        }
        pages
    }

    fn decode_text(data: &[u8]) -> String {
        if let Ok(s) = std::str::from_utf8(data) {
            return s.to_string();
        }

        use encoding_rs::GBK;
        let (result, _encoding_used, had_errors) = GBK.decode(data);
        if !had_errors {
            return result.to_string();
        }

        for encoding in &[encoding_rs::BIG5, encoding_rs::SHIFT_JIS, encoding_rs::EUC_JP] {
            let (result, _encoding_used, had_errors) = encoding.decode(data);
            if !had_errors {
                return result.to_string();
            }
        }

        String::from_utf8_lossy(data).to_string()
    }

    fn strip_html(html: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;
        for c in html.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => result.push(c),
                _ => {}
            }
        }
        result
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Document for ReflowDocument {
    fn page_count(&self) -> usize {
        self.chapters.len()
    }

    fn page_text(&self, page: usize) -> String {
        if page < self.chapters.len() {
            self.chapters[page].clone()
        } else {
            String::new()
        }
    }

    fn title(&self) -> String {
        self.doc_title.clone()
    }

    fn metadata(&self, _key: &str) -> Option<String> {
        None
    }

    fn render_page(&self, _page: usize, _scale: f32) -> Option<RenderedPage> {
        None
    }

    fn toc_entries(&self) -> Vec<TocEntry> {
        self.toc.clone()
    }
}
