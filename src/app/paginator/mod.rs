use crate::app::engines::ContentBlock;
use std::ops::Range;

/// Common book / page sizes in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PageSize {
    A4,       // 210×297 mm
    B5,       // 176×250 mm
    A5,       // 148×210 mm
    SixteenMo,// 170×230 mm
    ThirtyTwoMo,// 130×184 mm
    B6,       // 128×182 mm
    Letter,   // 215.9×279.4 mm
    Custom(f32, f32),
}

impl PageSize {
    pub fn width_mm(&self) -> f32 {
        match self {
            PageSize::A4 => 210.0,
            PageSize::B5 => 176.0,
            PageSize::A5 => 148.0,
            PageSize::SixteenMo => 170.0,
            PageSize::ThirtyTwoMo => 130.0,
            PageSize::B6 => 128.0,
            PageSize::Letter => 215.9,
            PageSize::Custom(w, _) => *w,
        }
    }

    pub fn height_mm(&self) -> f32 {
        match self {
            PageSize::A4 => 297.0,
            PageSize::B5 => 250.0,
            PageSize::A5 => 210.0,
            PageSize::SixteenMo => 230.0,
            PageSize::ThirtyTwoMo => 184.0,
            PageSize::B6 => 182.0,
            PageSize::Letter => 279.4,
            PageSize::Custom(_, h) => *h,
        }
    }

    /// Convert to egui points.  1pt = 1/72 inch, 1 mm = 1/25.4 inch.
    pub fn width_pt(&self) -> f32 { self.width_mm() / 25.4 * 72.0 }
    pub fn height_pt(&self) -> f32 { self.height_mm() / 25.4 * 72.0 }

    pub fn name(&self) -> &'static str {
        match self {
            PageSize::A4 => "A4",
            PageSize::B5 => "B5",
            PageSize::A5 => "A5",
            PageSize::SixteenMo => "16开",
            PageSize::ThirtyTwoMo => "32开",
            PageSize::B6 => "B6",
            PageSize::Letter => "Letter",
            PageSize::Custom(_, _) => "Custom",
        }
    }
}

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
    page_size: PageSize,
    font_size: f32,
    pages: Vec<PageLayout>,
    /// When true, the page list was built with the old chapter-to-page heuristic
    /// and must be rebuilt with `repaginate_with_fonts()` before rendering.
    needs_repaginate: bool,
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

/// Margins in points (left, right, top, bottom).
const MARGIN_LR: f32 = 28.0;
const MARGIN_TB: f32 = 28.0;

impl Paginator {
    pub fn new(chapters: Vec<(String, Vec<ContentBlock>)>, page_size: PageSize, font_size: f32) -> Self {
        let chapters: Vec<ChapterContent> = chapters.into_iter()
            .map(|(title, blocks)| {
                let char_count: usize = blocks.iter()
                    .map(|b| match b {
                        ContentBlock::Text(t) => t.chars().count(),
                        ContentBlock::Image(_) => 1,
                    })
                    .sum();
                ChapterContent { title, blocks, char_count }
            })
            .collect();

        let mut p = Self {
            chapters,
            page_size,
            font_size,
            pages: Vec::new(),
            needs_repaginate: true,
        };
        p.repaginate_fallback();
        p
    }

    pub fn set_page_size(&mut self, ps: PageSize) {
        self.page_size = ps;
        self.needs_repaginate = true;
    }

    pub fn set_font_size(&mut self, size: f32) {
        if (size - self.font_size).abs() > 0.1 {
            self.font_size = size;
            self.needs_repaginate = true;
        }
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn needs_repaginate(&self) -> bool {
        self.needs_repaginate
    }

    pub fn page_size_width_pt(&self) -> f32 {
        self.page_size.width_pt()
    }

    pub fn page_size_height_pt(&self) -> f32 {
        self.page_size.height_pt()
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
        (p.chapter_idx, p.char_start.saturating_sub(before), p.char_end.saturating_sub(before))
    }

    /// Fallback: one page per chapter (plus one per image).
    /// Used before fonts are available.
    fn repaginate_fallback(&mut self) {
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
                        if !current_page_entries.is_empty() {
                            self.pages.push(PageLayout {
                                chapter_idx: ci,
                                char_start: 0,
                                char_end: current_page_chars,
                                entries: std::mem::take(&mut current_page_entries),
                            });
                        }
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

        if !current_page_entries.is_empty() {
            self.pages.push(PageLayout {
                chapter_idx: current_chapter_idx,
                char_start: 0,
                char_end: current_page_chars,
                entries: current_page_entries,
            });
        }
    }

    /// Real text-wrapping pagination using egui's font metrics.
    /// `fonts`: egui font context (from `ui.fonts(|f| f)`).
    /// `available_w`, `available_h`: content area in points.
    /// Returns `true` if page count changed (caller should reset caches).
    pub fn repaginate_with_fonts(&mut self, fonts: &egui::text::Fonts, available_w: f32, available_h: f32) -> bool {
        self.needs_repaginate = false;

        let old_count = self.pages.len();
        self.pages.clear();
        if self.chapters.is_empty() {
            return true;
        }

        let page_w = available_w - 2.0 * MARGIN_LR;
        let page_h = available_h - 2.0 * MARGIN_TB;
        // Guard against degenerate dimensions.
        if page_w <= 10.0 || page_h <= 10.0 {
            self.repaginate_fallback();
            return self.pages.len() != old_count;
        }

        let font_size = self.font_size;
        let line_spacing = font_size * 1.6;

        let mut current_entries: Vec<PageEntry> = Vec::new();
        let mut current_entry_char_start: usize = 0; // char start of the first entry on this page
        let mut current_chapter_idx: usize = 0;
        let mut page_local_char_offset: usize = 0; // running char count for this page only
        let mut global_char: usize = 0;

        for (ci, chapter) in self.chapters.iter().enumerate() {
            if chapter.blocks.is_empty() {
                continue;
            }

            // Hard chapter break: flush current page before new chapter (except the very first page)
            if ci > 0 && !current_entries.is_empty() {
                self.pages.push(PageLayout {
                    chapter_idx: current_chapter_idx,
                    char_start: current_entry_char_start,
                    char_end: current_entry_char_start + page_local_char_offset,
                    entries: std::mem::take(&mut current_entries),
                });
                page_local_char_offset = 0;
            }

            current_chapter_idx = ci;

            for (bi, block) in chapter.blocks.iter().enumerate() {
                match block {
                    ContentBlock::Text(t) => {
                        let text: &str = t;
                        let total_chars = text.chars().count();
                        if total_chars == 0 {
                            continue;
                        }
                        let mut offset: usize = 0;
                        while offset < total_chars {
                            // Build a layout job for the remaining text of this block.
                            let remaining: String = text.chars().skip(offset).collect();
                            if remaining.is_empty() {
                                break;
                            }

                            let remaining_len = remaining.len();
                            let layout_job = egui::epaint::text::LayoutJob {
                                text: remaining,
                                sections: vec![egui::epaint::text::LayoutSection {
                                    leading_space: 0.0,
                                    byte_range: 0..remaining_len,
                                    format: egui::epaint::text::TextFormat {
                                        font_id: egui::FontId::proportional(font_size),
                                        color: egui::Color32::WHITE,
                                        ..Default::default()
                                    },
                                }],
                                wrap: egui::epaint::text::TextWrapping {
                                    max_width: page_w,
                                    max_rows: 0,
                                    break_anywhere: true,
                                    overflow_character: None,
                                },
                                break_on_newline: true,
                                halign: egui::Align::LEFT,
                                justify: false,
                                first_row_min_height: 0.0,
                                round_output_to_gui: true,
                            };

                            let galley = fonts.layout_job(layout_job);
                            let galley_rows = &galley.rows;

                            if galley_rows.is_empty() {
                                break;
                            }

                            // Find how many rows fit within remaining page height.
                            let mut rows_fit: usize = 0;
                            let mut used_y: f32 = 0.0;
                            for row in galley_rows.iter() {
                                let row_h = row.height().max(line_spacing);
                                if used_y + row_h > page_h && rows_fit > 0 {
                                    break;
                                }
                                rows_fit += 1;
                                used_y += row_h;
                            }

                            if rows_fit == 0 && !galley_rows.is_empty() {
                                // At least force one row so we don't infinitely loop.
                                rows_fit = 1;
                            }

                            // Count characters in the rows that fit.
                            let chars_in_rows: usize = galley_rows[..rows_fit].iter()
                                .map(|r| r.char_count_excluding_newline().max(1))
                                .sum();

                            // Push this slice as a page entry.
                            if current_entries.is_empty() {
                                current_entry_char_start = global_char;
                            }
                            current_entries.push(PageEntry {
                                block_idx: bi,
                                char_range: offset..offset + chars_in_rows,
                            });
                            page_local_char_offset += chars_in_rows;
                            global_char += chars_in_rows;
                            offset += chars_in_rows;

                            // If the text had more rows than fit, the page is full.
                            if rows_fit < galley_rows.len() {
                                self.pages.push(PageLayout {
                                    chapter_idx: current_chapter_idx,
                                    char_start: current_entry_char_start,
                                    char_end: current_entry_char_start + page_local_char_offset,
                                    entries: std::mem::take(&mut current_entries),
                                });
                                page_local_char_offset = 0;
                                // Layout will start a new page on the next iteration.
                            }
                        }
                    }
                    ContentBlock::Image(img) => {
                        let max_img_w = page_w;
                        let img_h = if img.width > max_img_w as u32 && max_img_w > 0.0 {
                            (max_img_w / img.width as f32) * img.height as f32
                        } else {
                            img.height as f32
                        };

                        // If the image doesn't fit on the current page, flush first.
                        if !current_entries.is_empty() && img_h + line_spacing > page_h {
                            self.pages.push(PageLayout {
                                chapter_idx: current_chapter_idx,
                                char_start: current_entry_char_start,
                                char_end: current_entry_char_start + page_local_char_offset,
                                entries: std::mem::take(&mut current_entries),
                            });
                            page_local_char_offset = 0;
                        }

                        if current_entries.is_empty() {
                            current_entry_char_start = global_char;
                        }
                        current_entries.push(PageEntry {
                            block_idx: bi,
                            char_range: 0..1,
                        });
                        page_local_char_offset += 1;
                        global_char += 1;

                        // Always give image its own page (flush immediately).
                        self.pages.push(PageLayout {
                            chapter_idx: current_chapter_idx,
                            char_start: current_entry_char_start,
                            char_end: current_entry_char_start + page_local_char_offset,
                            entries: std::mem::take(&mut current_entries),
                        });
                        page_local_char_offset = 0;
                    }
                }
            }
        }

        // Flush remaining page.
        if !current_entries.is_empty() {
            self.pages.push(PageLayout {
                chapter_idx: current_chapter_idx,
                char_start: current_entry_char_start,
                char_end: current_entry_char_start + page_local_char_offset,
                entries: current_entries,
            });
        }

        self.pages.len() != old_count
    }
}
