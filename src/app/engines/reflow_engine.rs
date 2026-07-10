use super::{Document, ReflowLayout, TocEntry, StoredImage, ContentBlock, Chapter, BlockInfo, ChapterInfo};
use std::collections::{HashMap, HashSet};
use std::io::Read;

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
        } else if lower.ends_with(".txt") || lower.ends_with(".md") {
            Self::open_text_file(path)
        } else if lower.ends_with(".docx") {
            Self::open_docx(path)
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

        // Build id → spine index map from spine items
        let mut id_to_spine_idx: HashMap<String, usize> = HashMap::new();
        for (i, (id, _)) in spine_items.iter().enumerate() {
            id_to_spine_idx.insert(id.clone(), i);
        }

        // Build ToC — page_index = spine index (not flattened TOC index)
        let mut toc: Vec<TocEntry> = Vec::new();
        let toc_data = epub.toc();
        if let Some(contents) = toc_data.contents() {
            for (i, entry) in contents.flatten().enumerate() {
                let page_index = entry.manifest_entry()
                    .and_then(|me| id_to_spine_idx.get(me.id()).copied())
                    .unwrap_or(i);
                toc.push(TocEntry {
                    label: entry.label().to_string(),
                    page_index,
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

    fn open_text_file(path: &str) -> Option<Self> {
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

        let is_md = path.to_lowercase().ends_with(".md");

        // For markdown: detect level-1 (# ) headings BEFORE stripping,
        // so the chapter splitter can find them (after stripping, # markers are gone).
        let md_headings: Vec<(usize, String)> = if is_md {
            content.lines().enumerate()
                .filter_map(|(i, line)| {
                    let trimmed = line.trim();
                    if trimmed.starts_with("# ") && !trimmed[2..].trim().is_empty() {
                        Some((i, trimmed[2..].trim().to_string()))
                    } else if trimmed == "#" {
                        None  // bare # is not a heading
                    } else if trimmed.starts_with('#') && !trimmed.starts_with("# ") {
                        // higher-level headings (##, ###) — skip, let the body handle them
                        None
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            vec![]
        };

        let content = if is_md { Self::strip_markdown(&content) } else { content };

        // Split text into chapters by heading patterns or blank lines
        let (chapters, toc) = if is_md && !md_headings.is_empty() {
            Self::split_txt_at_lines(&content, &md_headings)
        } else {
            Self::split_txt_chapters(&content)
        };

        // Pre-populate cache with chapter blocks
        let mut cache = HashMap::new();
        for (i, (_, text)) in chapters.iter().enumerate() {
            cache.insert(i, vec![ContentBlock::Text(text.clone())]);
        }

        Some(Self {
            path: path.to_string(),
            doc_title,
            toc,
            epub: None,
            spine_items: vec![],
            chapter_cache: std::sync::Mutex::new(cache),
            image_cache: std::sync::Mutex::new(HashMap::new()),
        })
    }

    /// Split text into chapters at known heading line positions.
    /// Headings are prepended to each chapter body.
    fn split_txt_at_lines(text: &str, headings: &[(usize, String)]) -> (Vec<(String, String)>, Vec<TocEntry>) {
        let lines: Vec<&str> = text.lines().collect();
        let mut chapters: Vec<(String, String)> = Vec::new();
        let mut prev = 0;
        for &(hi, ref label) in headings {
            if hi > prev {
                let body: String = lines[prev..hi].iter().map(|l| *l).collect::<Vec<&str>>().join("\n").trim().to_string();
                if !body.is_empty() {
                    chapters.push((String::new(), body));
                }
            }
            chapters.push((label.clone(), String::new()));
            prev = hi + 1;
        }
        if prev < lines.len() {
            let body: String = lines[prev..].iter().map(|l| *l).collect::<Vec<&str>>().join("\n").trim().to_string();
            if !body.is_empty() {
                chapters.push((String::new(), body));
            }
        }

        // Merge heading-only chapters with their body
        let mut merged: Vec<(String, String)> = Vec::new();
        for (label, body) in chapters {
            if !label.is_empty() {
                if body.is_empty() {
                    merged.push((label, String::new()));
                } else {
                    merged.push((label, body));
                }
            } else {
                if let Some(last) = merged.last_mut() {
                    if last.1.is_empty() {
                        last.1 = format!("{}\n{}", last.0, body);
                        continue;
                    }
                }
                merged.push((String::new(), body));
            }
        }
        merged.retain(|(_, b)| !b.is_empty());

        let toc: Vec<TocEntry> = merged.iter().enumerate()
            .map(|(i, (label, _))| TocEntry { label: label.clone(), page_index: i })
            .collect();
        (merged, toc)
    }

    fn strip_markdown(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            // Code fence ``` … ```
            if i + 2 < len && chars[i] == '`' && chars[i+1] == '`' && chars[i+2] == '`' {
                i += 3;
                while i < len && !(i + 2 < len && chars[i] == '`' && chars[i+1] == '`' && chars[i+2] == '`') {
                    out.push(chars[i]);
                    i += 1;
                }
                i += 3; // skip closing ```
                continue;
            }
            // Inline code `…`
            if chars[i] == '`' {
                i += 1;
                while i < len && chars[i] != '`' {
                    out.push(chars[i]);
                    i += 1;
                }
                if i < len { i += 1; } // skip closing `
                continue;
            }
            // Image ![alt](url) — drop entirely
            if i + 1 < len && chars[i] == '!' && chars[i+1] == '[' {
                i += 2;
                while i < len && chars[i] != ']' { i += 1; }
                if i < len { i += 1; } // skip ]
                if i < len && chars[i] == '(' {
                    i += 1;
                    while i < len && chars[i] != ')' { i += 1; }
                    if i < len { i += 1; } // skip )
                }
                continue;
            }
            // Link [text](url) — keep text only
            if chars[i] == '[' {
                let start = i + 1;
                let mut depth = 1;
                let mut j = start;
                while j < len && depth > 0 {
                    if chars[j] == '[' { depth += 1; }
                    else if chars[j] == ']' { depth -= 1; }
                    j += 1;
                }
                let text_end = if depth == 0 { j - 1 } else { i + 1 };
                // check for following (url)
                if text_end + 1 < len && chars[text_end + 1] == '(' {
                    // output link text
                    for k in start..text_end {
                        out.push(chars[k]);
                    }
                    i = text_end + 1;
                    while i < len && chars[i] != ')' { i += 1; }
                    if i < len { i += 1; }
                } else {
                    // not a link, just output [
                    out.push('[');
                    i = start;
                }
                continue;
            }
            // Strikethrough ~~text~~ or bold **text** or italic *text*
            // Handle ~~ before ** before * to avoid partial matches
            if i + 1 < len && chars[i] == '~' && chars[i+1] == '~' {
                i += 2;
                while i + 1 < len && !(chars[i] == '~' && chars[i+1] == '~') {
                    out.push(chars[i]);
                    i += 1;
                }
                if i + 1 < len { i += 2; } // skip ~~
                continue;
            }
            if i + 1 < len && chars[i] == '*' && chars[i+1] == '*' {
                i += 2;
                while i + 1 < len && !(chars[i] == '*' && chars[i+1] == '*') {
                    out.push(chars[i]);
                    i += 1;
                }
                if i + 1 < len { i += 2; } // skip **
                continue;
            }
            if chars[i] == '*' {
                i += 1;
                while i < len && chars[i] != '*' {
                    out.push(chars[i]);
                    i += 1;
                }
                if i < len { i += 1; } // skip *
                continue;
            }
            // Blockquote >
            if chars[i] == '>' && (i == 0 || chars[i-1] == '\n') {
                i += 1;
                if i < len && chars[i] == ' ' { i += 1; }
                continue;
            }
            // Heading markers # — strip them (they become chapter boundaries later)
            if chars[i] == '#' && (i == 0 || chars[i-1] == '\n') {
                i += 1;
                while i < len && (chars[i] == '#' || chars[i] == ' ') { i += 1; }
                continue;
            }
            out.push(chars[i]);
            i += 1;
        }

        out
    }

    fn open_docx(path: &str) -> Option<Self> {
        let data = std::fs::read(path).ok()?;
        let cursor = std::io::Cursor::new(&data);
        let mut archive = zip::ZipArchive::new(cursor).ok()?;

        let mut document_xml = archive.by_name("word/document.xml").ok()?;
        let mut xml_bytes = Vec::new();
        document_xml.read_to_end(&mut xml_bytes).ok()?;

        let doc_title = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        let (chapters, toc) = Self::parse_docx_xml(&xml_bytes);

        if chapters.is_empty() {
            return None;
        }

        let mut cache = HashMap::new();
        for (i, (_, text)) in chapters.iter().enumerate() {
            cache.insert(i, vec![ContentBlock::Text(text.clone())]);
        }

        Some(Self {
            path: path.to_string(),
            doc_title,
            toc,
            epub: None,
            spine_items: vec![],
            chapter_cache: std::sync::Mutex::new(cache),
            image_cache: std::sync::Mutex::new(HashMap::new()),
        })
    }

    fn parse_docx_xml(xml: &[u8]) -> (Vec<(String, String)>, Vec<TocEntry>) {
        use quick_xml::events::Event;
        use quick_xml::Reader;

        let mut reader = Reader::from_reader(xml);
        reader.config_mut().trim_text(true);

        let mut chapters: Vec<(String, String)> = Vec::new();
        let mut current_body = String::new();
        let mut is_heading = false;
        let mut heading_text = String::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if tag.ends_with(":pStyle") || tag == "w:pStyle" {
                        if let Ok(Some(val)) = e.try_get_attribute(b"w:val") {
                            let v = String::from_utf8_lossy(&val.value).to_string();
                            is_heading = v.starts_with("Heading") || v == "heading" || v.starts_with("heading");
                        }
                    }
                }
                Ok(Event::Text(ref e)) => {
                    let text = e.unescape().unwrap_or_default().to_string();
                    current_body.push_str(&text);
                }
                Ok(Event::End(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if tag.ends_with(":p") || tag == "w:p" {
                        let trimmed = current_body.trim().to_string();
                        if !trimmed.is_empty() {
                            if is_heading {
                                heading_text = trimmed;
                            } else {
                                if !heading_text.is_empty() {
                                    chapters.push((heading_text.clone(), String::new()));
                                    heading_text.clear();
                                }
                                chapters.push((String::new(), trimmed));
                            }
                        }
                        current_body.clear();
                        is_heading = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        // Merge heading-only chapters with following body chapter
        let mut merged: Vec<(String, String)> = Vec::new();
        for (label, body) in &chapters {
            if !label.is_empty() {
                if body.is_empty() {
                    merged.push((label.clone(), String::new()));
                } else {
                    merged.push((label.clone(), body.clone()));
                }
            } else {
                if let Some(last) = merged.last_mut() {
                    if !last.0.is_empty() && last.1.is_empty() {
                        last.1 = format!("{}\n{}", last.0, body);
                        continue;
                    }
                }
                merged.push((String::new(), body.clone()));
            }
        }
        merged.retain(|(_, b)| !b.is_empty());

        let toc: Vec<TocEntry> = merged.iter().enumerate()
            .filter(|(_, (label, _))| !label.is_empty())
            .map(|(i, (label, _))| TocEntry { label: label.clone(), page_index: i })
            .collect();

        (merged, toc)
    }

    fn split_txt_chapters(text: &str) -> (Vec<(String, String)>, Vec<TocEntry>) {
        let lines: Vec<&str> = text.lines().collect();

        // Phase 1: detect chapter heading line indices + labels
        let headings: Vec<(usize, String)> = lines.iter().enumerate()
            .filter_map(|(i, line)| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return None;
                }
                // Markdown heading
                if trimmed.starts_with('#') && trimmed.trim_start_matches('#').trim().len() > 0 {
                    return Some((i, trimmed.trim_start_matches('#').trim().to_string()));
                }
                // Chinese chapter: 第...章, 第...回, 第...节
                if trimmed.len() > 2 && trimmed.chars().next() == Some('第') && (trimmed.contains('章') || trimmed.contains('回') || trimmed.contains('节')) {
                    return Some((i, trimmed.to_string()));
                }
                // English chapter: "Chapter " or "CHAPTER "
                let lower = trimmed.to_lowercase();
                if lower.starts_with("chapter ") || lower.starts_with("chapter:") {
                    return Some((i, trimmed.to_string()));
                }
                None
            })
            .collect();

        // Phase 2: fallback — split on 2+ consecutive blank lines
        if headings.is_empty() {
            let breaks: Vec<usize> = lines.windows(3).enumerate()
                .filter(|(_, w)| w[0].trim().is_empty() && w[1].trim().is_empty())
                .map(|(i, _)| i)
                .collect();

            if breaks.is_empty() {
                return (vec![(String::new(), text.to_string())], vec![]);
            }

            let mut chapters: Vec<(String, String)> = Vec::new();
            let mut prev = 0;
            for &b in &breaks {
                let chunk: String = lines[prev..b].iter().map(|l| *l).collect::<Vec<&str>>().join("\n").trim().to_string();
                if !chunk.is_empty() {
                    chapters.push((format!("Part {}", chapters.len() + 1), chunk));
                }
                prev = b;
            }
            // remaining text after last break
            // advance past blank lines
            while prev < lines.len() && lines[prev].trim().is_empty() { prev += 1; }
            if prev < lines.len() {
                let chunk: String = lines[prev..].iter().map(|l| *l).collect::<Vec<&str>>().join("\n").trim().to_string();
                if !chunk.is_empty() {
                    chapters.push((format!("Part {}", chapters.len() + 1), chunk));
                }
            }

            let toc: Vec<TocEntry> = chapters.iter().enumerate()
                .map(|(i, (label, _))| TocEntry { label: label.clone(), page_index: i })
                .collect();
            return (chapters, toc);
        }

        // Phase 3: split at headings (heading line becomes chapter label, excluded from body)
        let mut chapters: Vec<(String, String)> = Vec::new();
        let mut prev = 0; // line index after the previous heading
        for &(hi, ref label) in &headings {
            if hi > prev {
                let body: String = lines[prev..hi].iter().map(|l| *l).collect::<Vec<&str>>().join("\n").trim().to_string();
                if !body.is_empty() {
                    chapters.push((String::new(), body));
                }
            }
            chapters.push((label.clone(), String::new()));
            prev = hi + 1;
        }
        if prev < lines.len() {
            let body: String = lines[prev..].iter().map(|l| *l).collect::<Vec<&str>>().join("\n").trim().to_string();
            if !body.is_empty() {
                chapters.push((String::new(), body));
            }
        }

        // Merge consecutive heading-only chapters with their body.
        // Include the heading label at the start of the body so it appears in rendered text.
        let mut merged: Vec<(String, String)> = Vec::new();
        for (label, body) in chapters {
            if !label.is_empty() {
                if body.is_empty() {
                    merged.push((label, String::new()));
                } else {
                    merged.push((label, body));
                }
            } else {
                if let Some(last) = merged.last_mut() {
                    if last.1.is_empty() {
                        // Prepend heading label so it appears in the rendered content.
                        last.1 = format!("{}\n{}", last.0, body);
                        continue;
                    }
                }
                merged.push((String::new(), body));
            }
        }
        merged.retain(|(_, b)| !b.is_empty());

        let toc: Vec<TocEntry> = merged.iter().enumerate()
            .map(|(i, (label, _))| TocEntry { label: label.clone(), page_index: i })
            .collect();
        (merged, toc)
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

    /// Read and parse a chapter's HTML into raw blocks, collecting image references.
    fn parse_chapter_raw_blocks(&self, chapter_idx: usize) -> (Vec<RawBlock>, HashSet<String>) {
        let (_, href) = match self.spine_items.get(chapter_idx) {
            Some(item) => item,
            None => return (vec![], HashSet::new()),
        };

        let epub_guard = match self.epub.as_ref().map(|m| m.lock().unwrap()) {
            Some(g) => g,
            None => return (vec![], HashSet::new()),
        };

        let html_bytes = match epub_guard.read_resource_bytes(href.as_str()) {
            Ok(b) => b,
            Err(_) => return (vec![], HashSet::new()),
        };
        let html = String::from_utf8_lossy(&html_bytes).into_owned();

        let mut referenced = HashSet::new();
        let raw_blocks = Self::extract_raw_blocks(&html, href, &mut referenced);
        (raw_blocks, referenced)
    }

    /// Load and parse a single chapter's full blocks (text + images). Result is cached.
    fn load_chapter_blocks(&self, chapter_idx: usize) -> Vec<ContentBlock> {
        {
            let cache = self.chapter_cache.lock().unwrap();
            if let Some(blocks) = cache.get(&chapter_idx) {
                return blocks.clone();
            }
        }

        let (raw_blocks, referenced) = self.parse_chapter_raw_blocks(chapter_idx);

        // Load image bytes for referenced images
        let epub_guard = self.epub.as_ref().map(|m| m.lock().unwrap());
        {
            let mut image_cache = self.image_cache.lock().unwrap();
            if let Some(ref epub) = epub_guard {
                for img_href in &referenced {
                    if image_cache.contains_key(img_href) {
                        continue;
                    }
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
        }

        // Convert RawBlocks → ContentBlocks
        let image_cache = self.image_cache.lock().unwrap();
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

    /// Read image dimensions from EPUB archive without storing raw_bytes.
    fn read_image_dimensions(
        epub: &Option<std::sync::Mutex<rbook::Epub>>,
        hrefs: &HashSet<String>,
    ) -> HashMap<String, (u32, u32)> {
        let mut dims = HashMap::new();
        let epub_guard = match epub.as_ref().map(|m| m.lock().unwrap()) {
            Some(g) => g,
            None => return dims,
        };
        for href in hrefs {
            let path = if href.starts_with('/') { href.clone() } else { format!("/{}", href) };
            if let Ok(bytes) = epub_guard.read_resource_bytes(&path) {
                let (w, h) = image::ImageReader::new(std::io::Cursor::new(&bytes))
                    .with_guessed_format()
                    .ok()
                    .and_then(|r| r.into_dimensions().ok())
                    .unwrap_or((0, 0));
                dims.insert(href.clone(), (w, h));
            }
        }
        dims
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

impl ReflowLayout for ReflowDocument {
    fn chapter_count(&self) -> usize {
        if self.spine_items.is_empty() {
            let cache = self.chapter_cache.lock().unwrap();
            let n = cache.len();
            if n > 0 { n } else { 1 }
        } else {
            self.spine_items.len()
        }
    }

    fn chapter_text(&self, idx: usize) -> String {
        let ch = self.load_chapter(idx);
        ch.blocks.iter()
            .map(|b| match b {
                ContentBlock::Text(t) => t.as_str(),
                ContentBlock::Image(_) => "[IMAGE]",
            })
            .collect::<Vec<&str>>()
            .join("\n")
    }

    fn load_chapter(&self, idx: usize) -> Chapter {
        if self.spine_items.is_empty() {
            let cache = self.chapter_cache.lock().unwrap();
            return cache.get(&idx).cloned().map(|blocks| Chapter {
                title: self.toc.get(idx).map(|t| t.label.clone()).unwrap_or_default(),
                blocks,
            }).unwrap_or_else(|| Chapter {
                title: self.toc.get(idx).map(|t| t.label.clone()).unwrap_or_default(),
                blocks: vec![],
            });
        }
        let blocks = self.load_chapter_blocks(idx);
        let title = self.toc.get(idx).map(|t| t.label.clone()).unwrap_or_default();
        Chapter { title, blocks }
    }

    fn chapter_info(&self, idx: usize) -> ChapterInfo {
        let title = self.toc.get(idx).map(|t| t.label.clone()).unwrap_or_default();
        if self.spine_items.is_empty() {
            // For text/docx files, use cached chapter data
            let cache = self.chapter_cache.lock().unwrap();
            let blocks: Vec<BlockInfo> = cache.get(&idx).map(|blocks| {
                blocks.iter().map(|b| match b {
                    ContentBlock::Text(t) => BlockInfo { is_image: false, char_count: t.chars().count() },
                    ContentBlock::Image(_) => BlockInfo { is_image: true, char_count: 1 },
                }).collect()
            }).unwrap_or_default();
            return ChapterInfo { title, blocks };
        }
        let (raw_blocks, _referenced) = self.parse_chapter_raw_blocks(idx);
        let blocks: Vec<BlockInfo> = raw_blocks.into_iter()
            .filter_map(|rb| match rb {
                RawBlock::Text(t) => {
                    let trimmed = t.trim().to_string();
                    if trimmed.is_empty() { None }
                    else { Some(BlockInfo { is_image: false, char_count: trimmed.chars().count() }) }
                }
                RawBlock::ImageRef(_) => {
                    Some(BlockInfo { is_image: true, char_count: 1 })
                }
            })
            .collect();
        ChapterInfo { title, blocks }
    }

    fn load_chapter_text_only(&self, idx: usize) -> Chapter {
        let title = self.toc.get(idx).map(|t| t.label.clone()).unwrap_or_default();
        if self.spine_items.is_empty() {
            return Chapter { title, blocks: vec![] };
        }
        let (raw_blocks, referenced) = self.parse_chapter_raw_blocks(idx);
        let dims = Self::read_image_dimensions(&self.epub, &referenced);
        let blocks: Vec<ContentBlock> = raw_blocks.into_iter()
            .filter_map(|rb| match rb {
                RawBlock::Text(t) => {
                    let trimmed = t.trim().to_string();
                    if trimmed.is_empty() { None } else { Some(ContentBlock::Text(trimmed)) }
                }
                RawBlock::ImageRef(href) => {
                    let (w, h) = dims.get(&href).copied().unwrap_or((600, 800));
                    Some(ContentBlock::Image(StoredImage {
                        raw_bytes: Vec::new(),
                        width: w,
                        height: h,
                    }))
                }
            })
            .collect();
        Chapter { title, blocks }
    }
}

impl Document for ReflowDocument {
    fn title(&self) -> String {
        self.doc_title.clone()
    }

    fn metadata(&self, _key: &str) -> Option<String> {
        None
    }

    fn toc_entries(&self) -> Vec<TocEntry> {
        self.toc.clone()
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
