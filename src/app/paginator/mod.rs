use crate::app::engines::ContentBlock;
use std::ops::Range;

/// A page entry describes which part of which chapter block is on this page.
#[derive(Clone)]
pub struct PageEntry {
    pub block_idx: usize,
    pub char_range: Range<usize>,
}

/// A paginator breaks chapter content into viewport-sized pages.
#[derive(Clone)]
pub struct Paginator {
    chapters: Vec<ChapterContent>,
    viewport_width: f32,
    viewport_height: f32,
    font_size: f32,
    pages: Vec<PageLayout>,
}

#[derive(Clone)]
struct ChapterContent {
    title: String,
    blocks: Vec<ContentBlock>,
    /// Total text length (characters) across all blocks in this chapter
    char_count: usize,
}

#[derive(Clone)]
struct PageLayout {
    chapter_idx: usize,
    char_start: usize,
    char_end: usize,
    /// Per-block character ranges on this page
    entries: Vec<PageEntry>,
}

impl Paginator {
    pub fn new(chapters: Vec<(String, Vec<ContentBlock>)>, viewport_width: f32, viewport_height: f32, font_size: f32) -> Self {
        let chapters: Vec<ChapterContent> = chapters.into_iter()
            .map(|(title, blocks)| {
                let char_count: usize = blocks.iter()
                    .map(|b| match b {
                        ContentBlock::Text(t) => t.chars().count(),
                        ContentBlock::Image(_) => 1, // counts as 1 "character height"
                    })
                    .sum();
                ChapterContent { title, blocks, char_count }
            })
            .collect();

        let mut p = Self {
            chapters,
            viewport_width,
            viewport_height,
            font_size,
            pages: Vec::new(),
        };
        p.repaginate();
        p
    }

    pub fn set_viewport(&mut self, w: f32, h: f32) {
        if (w - self.viewport_width).abs() > 0.5 || (h - self.viewport_height).abs() > 0.5 {
            self.viewport_width = w;
            self.viewport_height = h;
            self.repaginate();
        }
    }

    pub fn set_font(&mut self, size: f32) {
        if (size - self.font_size).abs() > 0.1 {
            self.font_size = size;
            self.repaginate();
        }
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn page_entries(&self, page: usize) -> &[PageEntry] {
        if page >= self.pages.len() {
            return &[];
        }
        &self.pages[page].entries
    }

    pub fn chapter_idx_for_page(&self, page: usize) -> Option<usize> {
        self.pages.get(page).map(|p| p.chapter_idx)
    }

    pub fn find_page_for_chapter(&self, chapter_idx: usize) -> usize {
        for (i, p) in self.pages.iter().enumerate() {
            if p.chapter_idx >= chapter_idx {
                return i;
            }
        }
        self.pages.len().saturating_sub(1)
    }

    pub fn find_page_for_char(&self, chapter_idx: usize, char_offset: usize) -> usize {
        // Find the first page that contains this chapter + character offset
        let mut running = 0;
        for ci in 0..chapter_idx {
            if ci < self.chapters.len() {
                running += self.chapters[ci].char_count;
            }
        }
        let global_offset = running + char_offset;
        for (i, p) in self.pages.iter().enumerate() {
            if p.char_end > global_offset {
                return i;
            }
        }
        self.pages.len().saturating_sub(1)
    }

    pub fn chapter_range_for_page(&self, page: usize) -> (usize, usize, usize) {
        // returns (chapter_idx, char_start_in_chapter, char_end_in_chapter)
        let p = match self.pages.get(page) {
            Some(p) => p,
            None => return (0, 0, 0),
        };
        let mut before = 0;
        for ci in 0..p.chapter_idx {
            if ci < self.chapters.len() {
                before += self.chapters[ci].char_count;
            }
        }
        (p.chapter_idx, p.char_start.saturating_sub(before), p.char_end.saturating_sub(before))
    }

    fn repaginate(&mut self) {
        self.pages.clear();
        if self.chapters.is_empty() {
            return;
        }

        let line_height = self.font_size * 1.6;
        let page_height = self.viewport_height;
        let chars_per_line = ((self.viewport_width - 40.0).max(100.0) / (self.font_size * 0.55)).max(10.0) as usize;
        let max_lines = if page_height > 0.0 { (page_height / line_height).floor() as usize } else { 9999 };
        let max_chars_per_page = max_lines * chars_per_line;

        let mut global_char_offset: usize = 0;
        let mut current_page_entries: Vec<PageEntry> = Vec::new();
        let mut current_page_chars: usize = 0;
        let mut current_chapter_idx: usize = 0;

        for (ci, chapter) in self.chapters.iter().enumerate() {
            if chapter.blocks.is_empty() {
                continue;
            }

            // Force a page break at each new chapter (except the first).
            // This ensures each chapter starts on its own page.
            if ci > 0 && !current_page_entries.is_empty() {
                self.pages.push(PageLayout {
                    chapter_idx: current_chapter_idx,
                    char_start: global_char_offset,
                    char_end: global_char_offset + current_page_chars,
                    entries: std::mem::take(&mut current_page_entries),
                });
                global_char_offset += current_page_chars;
                current_page_chars = 0;
            }

            // Walk through each block in the chapter
            for (bi, block) in chapter.blocks.iter().enumerate() {
                let (block_text, block_len) = match block {
                    ContentBlock::Text(t) => {
                        let len = t.chars().count();
                        (Some(t.as_str()), len)
                    }
                    ContentBlock::Image(_) => {
                        // Images take up one "page" worth of height
                        (None, 1)
                    }
                };

                let mut pos = 0;
                while pos < block_len {
                    let remaining = block_len - pos;
                    let space_on_page = if current_page_chars < max_chars_per_page {
                        max_chars_per_page - current_page_chars
                    } else {
                        0
                    };

                    if space_on_page == 0 && !current_page_entries.is_empty() {
                        // Start a new page
                        self.pages.push(PageLayout {
                            chapter_idx: current_chapter_idx,
                            char_start: global_char_offset,
                            char_end: global_char_offset + current_page_chars,
                            entries: std::mem::take(&mut current_page_entries),
                        });
                        global_char_offset += current_page_chars;
                        current_page_chars = 0;
                    }

                    let chunk = remaining.min(space_on_page.max(1));
                    if block_text.is_none() {
                        if current_page_chars > 0 {
                            // Flush current page first
                            self.pages.push(PageLayout {
                                chapter_idx: current_chapter_idx,
                                char_start: global_char_offset,
                                char_end: global_char_offset + current_page_chars,
                                entries: std::mem::take(&mut current_page_entries),
                            });
                        global_char_offset += current_page_chars;
                    }
                    current_chapter_idx = ci;
                    current_page_entries.push(PageEntry {
                        block_idx: bi,
                        char_range: pos..pos + 1,
                    });
                        current_page_chars = max_chars_per_page; // force page break after image
                        pos += 1;
                        continue;
                    }
                    current_chapter_idx = ci;
                    current_page_entries.push(PageEntry {
                        block_idx: bi,
                        char_range: pos..pos + chunk,
                    });
                    current_page_chars += chunk;
                    pos += chunk;
                }
            }
        }

        // Flush remaining page
        if !current_page_entries.is_empty() {
            self.pages.push(PageLayout {
                chapter_idx: current_chapter_idx,
                char_start: global_char_offset,
                char_end: global_char_offset + current_page_chars,
                entries: current_page_entries,
            });
        }
    }
}
