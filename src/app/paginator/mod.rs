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

        let mut current_page_entries: Vec<PageEntry> = Vec::new();
        let mut current_page_chars: usize = 0;
        let mut current_chapter_idx: usize = 0;

        for (ci, chapter) in self.chapters.iter().enumerate() {
            if chapter.blocks.is_empty() {
                continue;
            }

            // Force page break at each new chapter (except the first).
            if ci > 0 && !current_page_entries.is_empty() {
                self.pages.push(PageLayout {
                    chapter_idx: current_chapter_idx,
                    char_start: 0,
                    char_end: current_page_chars,
                    entries: std::mem::take(&mut current_page_entries),
                });
                current_page_chars = 0;
            }

            current_chapter_idx = ci;

            for (bi, block) in chapter.blocks.iter().enumerate() {
                match block {
                    ContentBlock::Text(t) => {
                        let len = t.chars().count();
                        current_page_entries.push(PageEntry {
                            block_idx: bi,
                            char_range: 0..len,
                        });
                        current_page_chars += len;
                    }
                    ContentBlock::Image(_) => {
                        // Flush any preceding text before the image
                        if !current_page_entries.is_empty() {
                            self.pages.push(PageLayout {
                                chapter_idx: ci,
                                char_start: 0,
                                char_end: current_page_chars,
                                entries: std::mem::take(&mut current_page_entries),
                            });
                        }
                        // Image gets its own page
                        current_page_entries.push(PageEntry {
                            block_idx: bi,
                            char_range: 0..1,
                        });
                        self.pages.push(PageLayout {
                            chapter_idx: ci,
                            char_start: 0,
                            char_end: 1,
                            entries: std::mem::take(&mut current_page_entries),
                        });
                        current_page_chars = 0;
                    }
                }
            }
        }

        // Flush remaining page
        if !current_page_entries.is_empty() {
            self.pages.push(PageLayout {
                chapter_idx: current_chapter_idx,
                char_start: 0,
                char_end: current_page_chars,
                entries: current_page_entries,
            });
        }
    }
}
