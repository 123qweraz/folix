use super::{Document, RenderedPage, TocEntry, StoredImage, ContentBlock};
use std::collections::{HashMap, HashSet};

/// Internal intermediate type: before images are loaded from the EPUB,
/// we only know their hrefs. After the second phase, these are converted
/// to `ContentBlock::Image` with actual byte data.
enum RawBlock {
    Text(String),
    ImageRef(String), // resolved href into EPUB's image map
}

pub struct ReflowDocument {
    path: String,
    full_text: String,
    doc_title: String,
    toc: Vec<TocEntry>,
    blocks: Vec<ContentBlock>,
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

        // Pre-build id → href map to avoid O(n²) manifest lookups per chapter
        let id_to_href: HashMap<String, String> = epub.manifest().iter()
            .map(|e| (e.id().to_string(), e.href().as_ref().to_string()))
            .collect();

        // ── Phase 1: Parse HTML to discover text & referenced image hrefs ──
        let mut full_text = String::new();
        let mut all_raw: Vec<(usize, RawBlock)> = Vec::new(); // (chapter_idx, block)
        let mut referenced_hrefs: HashSet<String> = HashSet::new();
        let mut chapter_char_offsets: Vec<usize> = Vec::new();

        let reader = epub.reader();
        let len = reader.len();

        for i in 0..len {
            chapter_char_offsets.push(full_text.chars().count());
            let Ok(result) = reader.get(i) else { continue };
            let html = result.content();
            let chapter_id = result.spine_entry().idref();

            let chapter_href = id_to_href
                .get(chapter_id)
                .cloned()
                .unwrap_or_default();

            let raw_blocks = Self::extract_raw_blocks(html, &chapter_href, &mut referenced_hrefs);

            // Build full_text from raw blocks
            for block in &raw_blocks {
                match block {
                    RawBlock::Text(t) => {
                        if i > 0 || !all_raw.is_empty() {
                            if !full_text.is_empty() && !full_text.ends_with('\n') {
                                full_text.push('\n');
                            }
                        }
                        full_text.push_str(t);
                    }
                    RawBlock::ImageRef(_) => {
                        full_text.push_str("[IMAGE]");
                    }
                }
            }

            if i + 1 < len && !full_text.ends_with('\n') {
                full_text.push('\n');
            }

            all_raw.extend(raw_blocks.into_iter().map(|b| (i, b)));
        }

        // ── Phase 2: Load image bytes ONLY for referenced images ──
        let mut image_data: HashMap<String, StoredImage> = HashMap::new();
        for entry in epub.manifest().iter() {
            let kind = entry.kind();
            if kind.is_image() {
                let href = normalize_path(entry.href().as_ref());
                if referenced_hrefs.contains(&href) {
                    if let Ok(bytes) = epub.read_resource_bytes(entry.href().as_ref()) {
                        let (w, h) = image::ImageReader::new(std::io::Cursor::new(&bytes))
                            .with_guessed_format()
                            .ok()
                            .and_then(|r| r.into_dimensions().ok())
                            .unwrap_or((0, 0));
                        image_data.insert(href, StoredImage {
                            raw_bytes: bytes,
                            width: w,
                            height: h,
                        });
                    }
                }
            }
        }

        // ── Phase 3: Convert RawBlocks → ContentBlocks ──
        let all_blocks: Vec<ContentBlock> = all_raw.into_iter()
            .filter_map(|(_, rb)| match rb {
                RawBlock::Text(t) => {
                    let trimmed = t.trim().to_string();
                    if trimmed.is_empty() { None } else { Some(ContentBlock::Text(trimmed)) }
                }
                RawBlock::ImageRef(href) => {
                    image_data.get(&href).map(|img| ContentBlock::Image(img.clone()))
                }
            })
            .collect();

        // Build ToC
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
            blocks: all_blocks,
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
            full_text: content.clone(),
            doc_title,
            toc: vec![],
            blocks: vec![ContentBlock::Text(content)],
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

    /// Parse HTML into raw blocks (text / image references), recording which
    /// images are actually used in `referenced_hrefs` for later byte loading.
    fn extract_raw_blocks(
        html: &str,
        chapter_href: &str,
        referenced_hrefs: &mut HashSet<String>,
    ) -> Vec<RawBlock> {
        let mut blocks = Vec::new();
        let mut current_text = String::new();
        let mut in_tag = false;
        let mut tag_content = String::new();

        for c in html.chars() {
            match c {
                '<' => {
                    in_tag = true;
                    tag_content.clear();
                }
                '>' => {
                    in_tag = false;
                    let tag_lower = tag_content.to_lowercase();

                    if tag_lower.starts_with("img ") || tag_lower == "img" {
                        Self::push_image_ref(&tag_content, &mut blocks, &mut current_text, chapter_href, referenced_hrefs);
                    } else if tag_lower.starts_with("image ") || tag_lower == "image" {
                        Self::push_image_ref(&tag_content, &mut blocks, &mut current_text, chapter_href, referenced_hrefs);
                    } else if tag_lower.starts_with("br") || tag_lower.starts_with("hr") {
                        current_text.push('\n');
                    } else if tag_lower.starts_with('/') {
                        let closing_tag = tag_lower.trim_start_matches('/').split_whitespace().next().unwrap_or("");
                        match closing_tag {
                            "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "blockquote" | "li" | "td" | "th" => {
                                if !current_text.ends_with('\n') {
                                    current_text.push('\n');
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ if !in_tag => {
                    current_text.push(c);
                }
                _ => {
                    if in_tag {
                        tag_content.push(c);
                    }
                }
            }
        }

        // Flush remaining text
        let trimmed = current_text.trim().to_string();
        if !trimmed.is_empty() {
            blocks.push(RawBlock::Text(trimmed));
        }

        blocks
    }

    /// Extract src from `<img>`/`<image>` tag, resolve path, record it, and push a `RawBlock::ImageRef`.
    fn push_image_ref(
        tag_content: &str,
        blocks: &mut Vec<RawBlock>,
        current_text: &mut String,
        chapter_href: &str,
        referenced_hrefs: &mut HashSet<String>,
    ) {
        if let Some(src) = Self::extract_attr(tag_content, "src") {
            if src.starts_with("data:") || src.contains("://") {
                return;
            }
            let resolved = resolve_path(chapter_href, &src);
            let trimmed = current_text.trim().to_string();
            if !trimmed.is_empty() {
                blocks.push(RawBlock::Text(trimmed));
                current_text.clear();
            }
            referenced_hrefs.insert(resolved.clone());
            blocks.push(RawBlock::ImageRef(resolved));
        }
    }

    /// Extract an attribute value from a tag string like `img src="foo.png"`.
    fn extract_attr(tag: &str, attr: &str) -> Option<String> {
        let lower = tag.to_lowercase();
        // Try double quotes
        let search = format!("{}=\"", attr.to_lowercase());
        if let Some(start) = lower.find(&search) {
            let value_start = start + search.len();
            if value_start < tag.len() {
                let remaining = &tag[value_start..];
                if let Some(end) = remaining.find('"') {
                    return Some(remaining[..end].to_string());
                }
            }
        }
        // Try single quotes
        let search = format!("{}='", attr.to_lowercase());
        if let Some(start) = lower.find(&search) {
            let value_start = start + search.len();
            if value_start < tag.len() {
                let remaining = &tag[value_start..];
                if let Some(end) = remaining.find('\'') {
                    return Some(remaining[..end].to_string());
                }
            }
        }
        None
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

    fn content_blocks(&self, _page: usize) -> Vec<ContentBlock> {
        self.blocks.clone()
    }
}

fn normalize_path(p: &str) -> String {
    let p = p.trim_start_matches('/');
    p.replace('\\', "/")
}

fn resolve_path(chapter_path: &str, src: &str) -> String {
    if src.starts_with('/') || src.contains("://") {
        return normalize_path(src);
    }
    let base = std::path::Path::new(chapter_path).parent().unwrap_or(std::path::Path::new(""));
    let joined = base.join(src);
    let normalized = normalize_path(joined.to_str().unwrap_or(src));
    clean_path(&normalized)
}

/// Resolve `..` and `.` segments in a path.
/// e.g. "OEBPS/Text/../Images/foo.jpeg" → "OEBPS/Images/foo.jpeg"
fn clean_path(p: &str) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "." => {}
            ".." => {
                segments.pop();
            }
            _ => {
                segments.push(seg);
            }
        }
    }
    segments.join("/")
}
