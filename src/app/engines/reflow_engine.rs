use super::{Document, RenderedPage, TocEntry};

pub struct ReflowDocument {
    path: String,
    full_text: String,
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

        let mut full_text = String::new();
        let mut chapter_texts: Vec<String> = Vec::new();

        for result in epub.reader() {
            let data = result.ok()?;
            let html = data.content();
            let plain = Self::strip_html(html).trim().to_string();
            if !plain.is_empty() {
                chapter_texts.push(plain);
            }
        }

        // Concatenate all chapters, record char offsets for each
        let mut chapter_char_offsets: Vec<usize> = Vec::new();
        for (i, ct) in chapter_texts.iter().enumerate() {
            chapter_char_offsets.push(full_text.len());
            if i > 0 {
                full_text.push('\n');
            }
            full_text.push_str(ct);
        }

        // Build ToC: each flattened entry maps to the chapter with same index
        let mut toc: Vec<TocEntry> = Vec::new();
        let toc_data = epub.toc();
        if let Some(contents) = toc_data.contents() {
            for (i, entry) in contents.flatten().enumerate() {
                let char_offset = chapter_char_offsets.get(i).copied().unwrap_or(0);
                toc.push(TocEntry {
                    label: entry.label().to_string(),
                    page_index: char_offset,
                });
            }
        }

        if full_text.is_empty() {
            full_text = "(empty document)".to_string();
        }

        Some(Self {
            path: path.to_string(),
            full_text,
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

        if content.is_empty() {
            return None;
        }

        Some(Self {
            path: path.to_string(),
            full_text: content,
            doc_title,
            toc: vec![],
        })
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
        1
    }

    fn page_text(&self, page: usize) -> String {
        if page == 0 { self.full_text.clone() } else { String::new() }
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
