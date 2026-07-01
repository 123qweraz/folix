use super::{Document, RenderedPage, TocEntry, StoredImage, ContentBlock};
use std::collections::{HashMap, HashSet};

enum RawBlock {
    Text(String),
    ImageRef(String),
}

pub struct ReflowDocument {
    path: String,
    doc_title: String,
    toc: Vec<TocEntry>,
    epub: Option<std::sync::Mutex<rbook::Epub>>,
    spine_items: Vec<(String, String)>, // (id, href)
    chapter_cache: std::sync::Mutex<HashMap<usize, Vec<ContentBlock>>>,
    image_cache: std::sync::Mutex<HashMap<String, StoredImage>>,
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

        // Build id → href map for spine resolution.
        // Keep the raw href (with leading /) because read_resource_bytes needs it.
        let mut id_to_href: HashMap<String, String> = HashMap::new();
        for entry in epub.manifest().iter() {
            id_to_href.insert(entry.id().to_string(), entry.href().as_ref().to_string());
        }

        // Build spine items: ordered list of (id, href) for chapters
        let mut spine_items: Vec<(String, String)> = Vec::new();
        let reader = epub.reader();
        for i in 0..reader.len() {
            let Ok(result) = reader.get(i) else { continue };
            let chapter_id = result.spine_entry().idref();
            if let Some(href) = id_to_href.get(chapter_id).cloned() {
                spine_items.push((chapter_id.to_string(), href));
            }
        }

        // Build ToC — page_index = chapter index for non-image docs
        let mut toc: Vec<TocEntry> = Vec::new();
        let toc_data = epub.toc();
        if let Some(contents) = toc_data.contents() {
            for (entry, chapter_idx) in contents.flatten().zip(0..) {
                toc.push(TocEntry {
                    label: entry.label().to_string(),
                    page_index: chapter_idx,
                });
            }
        }

        Some(Self {
            path: path.to_string(),
            doc_title,
            toc,
            epub: Some(std::sync::Mutex::new(epub)),
            spine_items,
            chapter_cache: std::sync::Mutex::new(HashMap::new()),
            image_cache: std::sync::Mutex::new(HashMap::new()),
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

        // Pre-populate cache with the full TXT content
        let mut cache = HashMap::new();
        cache.insert(0usize, vec![ContentBlock::Text(content)]);

        Some(Self {
            path: path.to_string(),
            doc_title,
            toc: vec![],
            epub: None,
            spine_items: vec![],
            chapter_cache: std::sync::Mutex::new(cache),
            image_cache: std::sync::Mutex::new(HashMap::new()),
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

    /// Load and parse a single chapter. Result is cached.
    fn load_chapter(&self, chapter_idx: usize) -> Vec<ContentBlock> {
        {
            let cache = self.chapter_cache.lock().unwrap();
            if let Some(blocks) = cache.get(&chapter_idx) {
                return blocks.clone();
            }
        }

        let (_, href) = match self.spine_items.get(chapter_idx) {
            Some(item) => item,
            None => return vec![],
        };

        let epub_guard = self.epub.as_ref().map(|m| m.lock().unwrap());
        let epub = match epub_guard {
            Some(ref e) => e,
            None => return vec![],
        };

        let html_bytes = match epub.read_resource_bytes(href.as_str()) {
            Ok(b) => b,
            Err(_) => return vec![],
        };
        let html = String::from_utf8_lossy(&html_bytes).into_owned();

        // Parse HTML into raw blocks + collect referenced images
        let mut referenced = HashSet::new();
        let raw_blocks = Self::extract_raw_blocks(&html, href, &mut referenced);

        // Load image bytes for images referenced in this chapter.
        // read_resource_bytes needs paths rooted with / (EPUB-absolute).
        // Our resolve_path produces paths without leading /, so prepend one.
        let mut image_cache = self.image_cache.lock().unwrap();
        for img_href in &referenced {
            if !image_cache.contains_key(img_href) {
                let epub_img_path = if img_href.starts_with('/') {
                    img_href.clone()
                } else {
                    format!("/{}", img_href)
                };
                if let Ok(bytes) = epub.read_resource_bytes(epub_img_path.as_str()) {
                    let (w, h) = image::ImageReader::new(std::io::Cursor::new(&bytes))
                        .with_guessed_format()
                        .ok()
                        .and_then(|r| r.into_dimensions().ok())
                        .unwrap_or((0, 0));
                    image_cache.insert(img_href.clone(), StoredImage {
                        raw_bytes: bytes,
                        width: w,
                        height: h,
                    });
                }
            }
        }
        drop(epub_guard); // release the mutex

        // Convert RawBlocks → ContentBlocks
        let blocks: Vec<ContentBlock> = raw_blocks.into_iter()
            .filter_map(|rb| match rb {
                RawBlock::Text(t) => {
                    let trimmed = t.trim().to_string();
                    if trimmed.is_empty() { None } else { Some(ContentBlock::Text(trimmed)) }
                }
                RawBlock::ImageRef(href) => {
                    image_cache.get(&href).map(|img| ContentBlock::Image(img.clone()))
                }
            })
            .collect();

        {
            let mut cache = self.chapter_cache.lock().unwrap();
            cache.insert(chapter_idx, blocks.clone());
        }

        blocks
    }

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

        let trimmed = current_text.trim().to_string();
        if !trimmed.is_empty() {
            blocks.push(RawBlock::Text(trimmed));
        }

        blocks
    }

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

    fn extract_attr(tag: &str, attr: &str) -> Option<String> {
        let lower = tag.to_lowercase();
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
        if self.spine_items.is_empty() {
            1 // TXT: single page
        } else {
            self.spine_items.len()
        }
    }

    fn page_text(&self, page: usize) -> String {
        let blocks = self.content_blocks(page);
        blocks.iter()
            .map(|b| match b {
                ContentBlock::Text(t) => t.as_str(),
                ContentBlock::Image(_) => "[IMAGE]",
            })
            .collect::<Vec<&str>>()
            .join("\n")
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

    fn content_blocks(&self, page: usize) -> Vec<ContentBlock> {
        if self.spine_items.is_empty() {
            let cache = self.chapter_cache.lock().unwrap();
            return cache.get(&page).cloned().unwrap_or_default();
        }
        self.load_chapter(page)
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
