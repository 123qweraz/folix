use super::{Document, RenderedPage, TocEntry, StoredImage, ContentBlock};
use std::collections::HashMap;

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

        // Build image map: normalized href → decoded StoredImage
        let mut image_map: HashMap<String, StoredImage> = HashMap::new();
        for entry in epub.manifest().iter() {
            let kind = entry.kind();
            if kind.is_image() {
                let href = entry.href().as_ref().to_string();
                if let Ok(bytes) = epub.read_resource_bytes(href.as_str()) {
                    // Probe dimensions from image headers (fast, no full decode)
                    let (w, h) = image::image_dimensions(&bytes).unwrap_or((0, 0));
                    let si = StoredImage {
                        raw_bytes: bytes,
                        width: w,
                        height: h,
                    };
                    image_map.insert(normalize_path(&href), si);
                }
            }
        }

        let mut full_text = String::new();
        let mut all_blocks: Vec<ContentBlock> = Vec::new();
        let mut chapter_char_offsets: Vec<usize> = Vec::new();

        // Use a single reader for the loop
        let reader = epub.reader();
        let len = reader.len();

        for i in 0..len {
            chapter_char_offsets.push(full_text.chars().count());
            let Ok(result) = reader.get(i) else { continue };
            let html = result.content();
            let chapter_id = result.spine_entry().idref().to_string();

            // Find the manifest entry for this chapter to get its href (for resolving relative image paths)
            let chapter_href = epub.manifest().iter()
                .find(|e| e.id() == chapter_id)
                .map(|e| e.href().as_ref().to_string())
                .unwrap_or_default();

            // Parse HTML into blocks
            let chapter_blocks = Self::parse_html(html, &chapter_href, &image_map);

            for block in &chapter_blocks {
                match block {
                    ContentBlock::Text(t) => {
                        if i > 0 || !all_blocks.is_empty() {
                            // Only prepend newline if there's previous content
                            if !full_text.is_empty() && !full_text.ends_with('\n') {
                                full_text.push('\n');
                            }
                        }
                        full_text.push_str(t);
                    }
                    ContentBlock::Image(_img) => {
                        // Add a placeholder marker for text-based operations
                        full_text.push_str("[IMAGE]");
                    }
                }
            }

            // Append a chapter separator
            if i + 1 < len && !full_text.ends_with('\n') {
                full_text.push('\n');
            }

            all_blocks.extend(chapter_blocks);
        }

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

    /// Parse HTML into text + image blocks.
    /// `chapter_href` is the path of the chapter within the EPUB (for resolving relative image src).
    /// `image_map` maps normalized hrefs to decoded images.
    fn parse_html(
        html: &str,
        chapter_href: &str,
        image_map: &HashMap<String, StoredImage>,
    ) -> Vec<ContentBlock> {
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

                    // Check for <img> tag
                    if tag_lower.starts_with("img ") || tag_lower == "img" {
                        if let Some(src) = Self::extract_attr(&tag_content, "src") {
                            let resolved = resolve_path(chapter_href, &src);
                            if let Some(img) = image_map.get(&resolved) {
                                // Flush accumulated text
                                let trimmed = current_text.trim().to_string();
                                if !trimmed.is_empty() {
                                    blocks.push(ContentBlock::Text(trimmed));
                                    current_text.clear();
                                }
                                blocks.push(ContentBlock::Image(img.clone()));
                            }
                        }
                    }
                    // Check for <br>, <hr> etc.
                    else if tag_lower.starts_with("br") || tag_lower.starts_with("hr") {
                        current_text.push('\n');
                    }
                    // Check for <p>, <div>, <h1>-<h6> etc.
                    else if tag_lower.starts_with('/') {
                        let closing_tag = tag_lower.trim_start_matches('/').split_whitespace().next().unwrap_or("");
                        matches!(
                            closing_tag,
                            "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "blockquote" | "li" | "td" | "th"
                        ).then(|| {
                            if !current_text.ends_with('\n') {
                                current_text.push('\n');
                            }
                        });
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
            blocks.push(ContentBlock::Text(trimmed));
        }

        blocks
    }

    /// Extract an attribute value from a tag string like `img src="foo.png"`.
    fn extract_attr(tag: &str, attr: &str) -> Option<String> {
        let lower = tag.to_lowercase();
        let search = format!("{}=\"", attr.to_lowercase());
        if let Some(start) = lower.find(&search) {
            let value_start = start + search.len();
            let remaining = &tag[value_start..];
            if let Some(end) = remaining.find('"') {
                return Some(remaining[..end].to_string());
            }
        }
        // Also try single quotes
        let search = format!("{}='", attr.to_lowercase());
        if let Some(start) = lower.find(&search) {
            let value_start = start + search.len();
            let remaining = &tag[value_start..];
            if let Some(end) = remaining.find('\'') {
                return Some(remaining[..end].to_string());
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
