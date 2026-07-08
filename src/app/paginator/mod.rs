use crate::app::engines::ContentBlock;
use std::ops::Range;

/// A page entry describes which part of which chapter block is on this page.
#[derive(Clone, Debug)]
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
    char_count: usize,
}

#[derive(Clone)]
struct PageLayout {
    chapter_idx: usize,
    char_start: usize,
    char_end: usize,
    entries: Vec<PageEntry>,
}

impl Paginator {
    pub fn new(
        chapters: Vec<(String, Vec<ContentBlock>)>,
        viewport_width: f32,
        viewport_height: f32,
        font_size: f32,
    ) -> Self {
        let chapters: Vec<ChapterContent> = chapters
            .into_iter()
            .map(|(title, blocks)| {
                let char_count: usize = blocks
                    .iter()
                    .map(|b| match b {
                        ContentBlock::Text(t) => t.chars().count(),
                        ContentBlock::Image(_) => 1,
                    })
                    .sum();
                ChapterContent {
                    title,
                    blocks,
                    char_count,
                }
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
        if (w - self.viewport_width).abs() > 0.5
            || (h - self.viewport_height).abs() > 0.5
        {
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
        (
            p.chapter_idx,
            p.char_start.saturating_sub(before),
            p.char_end.saturating_sub(before),
        )
    }

    /// Split chapter content into viewport-sized pages by character count estimation.
    fn repaginate(&mut self) {
        self.pages.clear();
        if self.chapters.is_empty() {
            return;
        }

        let line_height = self.font_size * 1.6;
        let chars_per_line = (self.viewport_width / self.font_size).max(1.0) as usize;
        let lines_per_page = (self.viewport_height / line_height).max(1.0) as usize;
        let chars_per_page = chars_per_line * lines_per_page;

        let mut current_entries: Vec<PageEntry> = Vec::new();
        let mut page_char_count: usize = 0;
        let mut page_char_start: usize = 0;
        let mut current_chapter_idx: usize = 0;
        let mut global_char: usize = 0;

        for (ci, chapter) in self.chapters.iter().enumerate() {
            if chapter.blocks.is_empty() {
                continue;
            }

            // Always start a new page at chapter boundary.
            if ci > 0 && !current_entries.is_empty() {
                self.pages.push(PageLayout {
                    chapter_idx: current_chapter_idx,
                    char_start: page_char_start,
                    char_end: page_char_start + page_char_count,
                    entries: std::mem::take(&mut current_entries),
                });
                page_char_count = 0;
            }

            current_chapter_idx = ci;

            for (bi, block) in chapter.blocks.iter().enumerate() {
                match block {
                    ContentBlock::Text(t) => {
                        let total = t.chars().count();
                        if total == 0 {
                            continue;
                        }
                        let mut offset: usize = 0;
                        while offset < total {
                            let page_remain = chars_per_page.saturating_sub(page_char_count);
                            if page_remain == 0 {
                                // Flush full page
                                self.pages.push(PageLayout {
                                    chapter_idx: current_chapter_idx,
                                    char_start: page_char_start,
                                    char_end: page_char_start + page_char_count,
                                    entries: std::mem::take(&mut current_entries),
                                });
                                page_char_count = 0;
                                continue;
                            }
                            let take = page_remain.min(total - offset);
                            if current_entries.is_empty() {
                                page_char_start = global_char;
                            }
                            current_entries.push(PageEntry {
                                block_idx: bi,
                                char_range: offset..offset + take,
                            });
                            page_char_count += take;
                            global_char += take;
                            offset += take;
                        }
                    }
                    ContentBlock::Image(_) => {
                        if !current_entries.is_empty() {
                            self.pages.push(PageLayout {
                                chapter_idx: current_chapter_idx,
                                char_start: page_char_start,
                                char_end: page_char_start + page_char_count,
                                entries: std::mem::take(&mut current_entries),
                            });
                            page_char_count = 0;
                        }
                        page_char_start = global_char;
                        current_entries.push(PageEntry {
                            block_idx: bi,
                            char_range: 0..1,
                        });
                        page_char_count += 1;
                        global_char += 1;
                        // Image always gets its own page
                        self.pages.push(PageLayout {
                            chapter_idx: current_chapter_idx,
                            char_start: page_char_start,
                            char_end: page_char_start + page_char_count,
                            entries: std::mem::take(&mut current_entries),
                        });
                        page_char_count = 0;
                    }
                }
            }
        }

        // Flush remaining page
        if !current_entries.is_empty() {
            self.pages.push(PageLayout {
                chapter_idx: current_chapter_idx,
                char_start: page_char_start,
                char_end: page_char_start + page_char_count,
                entries: current_entries,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// chars_per_page for 800x1000@16pt: (800/16)*(1000/25.6) = 50*39 = 1950
    #[test]
    fn test_small_block_one_page() {
        let chapters = vec![(
            "Ch1".to_string(),
            vec![ContentBlock::Text("Hello World".to_string())],
        )];
        let p = Paginator::new(chapters, 800.0, 1000.0, 16.0);
        assert_eq!(p.page_count(), 1);
        let e = p.page_entries(0);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].char_range, 0..11);
    }

    #[test]
    fn test_large_block_split_across_pages() {
        // 4000 chars, chars_per_page=1950 → 1950+1950+100 = 3 pages
        let text = "A".repeat(4000);
        let chapters = vec![(
            "Ch1".to_string(),
            vec![ContentBlock::Text(text)],
        )];
        let p = Paginator::new(chapters, 800.0, 1000.0, 16.0);
        assert_eq!(p.page_count(), 3);
        assert_eq!(p.page_entries(0)[0].char_range, 0..1950);
        assert_eq!(p.page_entries(1)[0].char_range, 1950..3900);
        assert_eq!(p.page_entries(2)[0].char_range, 3900..4000);
    }

    #[test]
    fn test_multi_chapter_each_starts_new_page() {
        let chapters = vec![
            (
                "Ch1".to_string(),
                vec![ContentBlock::Text("Hello".to_string())],
            ),
            (
                "Ch2".to_string(),
                vec![ContentBlock::Text("World".to_string())],
            ),
        ];
        let p = Paginator::new(chapters, 800.0, 1000.0, 16.0);
        assert_eq!(p.page_count(), 2);
        assert_eq!(p.page_entries(0)[0].char_range, 0..5);
        assert_eq!(p.page_entries(1)[0].char_range, 0..5);
    }

    #[test]
    fn test_empty_chapters() {
        let chapters: Vec<(String, Vec<ContentBlock>)> = vec![];
        let p = Paginator::new(chapters, 800.0, 1000.0, 16.0);
        assert_eq!(p.page_count(), 0);
    }

    #[test]
    fn test_empty_blocks_no_pages() {
        let chapters = vec![("Ch1".to_string(), vec![])];
        let p = Paginator::new(chapters, 800.0, 1000.0, 16.0);
        assert_eq!(p.page_count(), 0);
    }

    #[test]
    fn test_image_gets_own_page() {
        use crate::app::engines::StoredImage;
        let img = ContentBlock::Image(StoredImage {
            width: 100,
            height: 100,
            raw_bytes: vec![],
        });
        let chapters = vec![(
            "Ch1".to_string(),
            vec![
                ContentBlock::Text("Hello".to_string()),
                img,
                ContentBlock::Text("World".to_string()),
            ],
        )];
        let p = Paginator::new(chapters, 800.0, 1000.0, 16.0);
        // text before image → page 0, image alone → page 1, text after → page 2
        assert_eq!(p.page_count(), 3);
    }
}
