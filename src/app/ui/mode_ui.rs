use crate::app::engines::Document;
use crate::app::core::mode_system::{ReadingState, ReadingLayout, ViewMode, Bookmark, AutoState, AnnotateState};
use std::sync::Arc;
use parking_lot::Mutex;

pub fn render_reading(ui: &mut egui::Ui, document: &Arc<Mutex<Box<dyn Document>>>, rs: &mut ReadingState) {
    let supports_image = document.lock().supports_image();
    if !supports_image {
        rs.view_mode = ViewMode::Text;
    }

    // Ctrl+F opens sidebar and focuses search
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F)) {
        rs.show_sidebar = true;
        rs.search.show_search = true;
    }

    match rs.reading_layout {
        ReadingLayout::Paged => {
            if rs.view_mode == ViewMode::Image && supports_image {
                render_paged_image(ui, document, rs);
            } else {
                render_text_continuous(ui, document, rs);
            }
        }
        ReadingLayout::Scroll => {
            if rs.view_mode == ViewMode::Image && supports_image {
                let total = document.lock().page_count();

                // If Paged→Scroll transition (or external jump via ToC/bookmark),
                // compute scroll offset from current page.
                let initial_scroll = if total > 0 && rs.scroll_offset_y == 0.0 && rs.page > 0 {
                    let limit = rs.page.min(total - 1);
                    let mut y = 0.0;
                    let d = document.lock();
                    for i in 0..limit {
                        let (_, h) = d.page_size(i, rs.scale).unwrap_or((800.0, 1000.0));
                        y += h + 12.0;
                    }
                    Some(y)
                } else {
                    None
                };

                render_continuous_images(ui, document, &mut rs.page, rs.scale, total, initial_scroll, &mut rs.scroll_offset_y, "pdf_scroll_reading");
            } else {
                render_text_continuous(ui, document, rs);
            }
        }
    }
}

pub fn render_sidebar(ui: &mut egui::Ui, document: &Arc<Mutex<Box<dyn Document>>>, rs: &mut ReadingState, total: usize) {
    ui.heading("Sidebar");
    ui.separator();

    // Table of Contents
    egui::CollapsingHeader::new("📖 Table of Contents")
        .default_open(true)
        .show(ui, |ui| {
            let toc = document.lock().toc_entries();
            if toc.is_empty() {
                ui.label("No table of contents");
            } else {
                egui::ScrollArea::vertical()
                    .max_height(f32::INFINITY)
                    .show(ui, |ui| {
                        for entry in &toc {
                            let selected = rs.page == entry.page_index;
                            if ui.selectable_label(selected, &entry.label).clicked() {
                                let target = entry.page_index.min(total.saturating_sub(1));
                                rs.page = target;
                                // Reset scroll offset so continuous rendering jumps to this page
                                rs.scroll_offset_y = 0.0;
                            }
                        }
                    });
            }
        });

    ui.separator();

    // Search
    egui::CollapsingHeader::new("🔍 Search")
        .default_open(true)
        .show(ui, |ui| {
            let full_text = document.lock().page_text(0);
            let prev_query = rs.search.query.clone();
            ui.add(egui::TextEdit::singleline(&mut rs.search.query)
                .hint_text("Search text...")
                .desired_width(f32::INFINITY));

            if rs.search.query != prev_query {
                rs.search.matches.clear();
                rs.search.current_match = 0;
                if !rs.search.query.is_empty() {
                    let lower_query = rs.search.query.to_lowercase();
                    let mut search_start = 0;
                    while let Some(pos) = full_text[search_start..].to_lowercase().find(&lower_query) {
                        let byte_offset = search_start + pos;
                        let char_offset = full_text[..byte_offset].chars().count();
                        rs.search.matches.push(char_offset);
                        if let Some(c) = full_text[byte_offset..].chars().next() {
                            search_start = byte_offset + c.len_utf8();
                        } else {
                            break;
                        }
                        if search_start >= full_text.len() { break; }
                    }
                }
            }

            let total_matches = rs.search.matches.len();
            let current = rs.search.current_match;
            ui.horizontal(|ui| {
                if total_matches > 0 {
                    ui.label(format!("{}/{}", current + 1, total_matches));
                } else if !rs.search.query.is_empty() {
                    ui.label("0 matches");
                }
                let enabled = total_matches > 0;
                if ui.add_enabled(enabled, egui::Button::new("▲")).clicked() {
                    rs.search.current_match = if current == 0 { total_matches - 1 } else { current - 1 };
                }
                if ui.add_enabled(enabled, egui::Button::new("▼")).clicked() {
                    rs.search.current_match = if current + 1 >= total_matches { 0 } else { current + 1 };
                }
            });
        });

    ui.separator();

    // Bookmarks
    egui::CollapsingHeader::new("🔖 Bookmarks")
        .default_open(true)
        .show(ui, |ui| {
            let mut remove_idx: Option<usize> = None;
            for (idx, bm) in rs.bookmarks.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.selectable_label(false, format!("{} (p.{})", bm.label, bm.page + 1)).clicked() {
                        rs.page = bm.page;
                    }
                    if ui.button("×").clicked() {
                        remove_idx = Some(idx);
                    }
                });
            }
            if let Some(idx) = remove_idx {
                rs.bookmarks.remove(idx);
            }

            if ui.button("+ Add Bookmark").clicked() {
                let label = format!("Page {}", rs.page + 1);
                rs.bookmarks.push(Bookmark { page: rs.page, label });
            }
        });
}

fn render_text_continuous(ui: &mut egui::Ui, doc: &Arc<Mutex<Box<dyn Document>>>, rs: &mut ReadingState) {
    let text = {
        let d = doc.lock();
        d.page_text(0)
    };

    if rs.search.show_search && !rs.search.matches.is_empty() {
        // Show context around current match
        let match_pos = rs.search.matches[rs.search.current_match];
        let total_chars = text.chars().count();
        let context = 300;
        let start = match_pos.saturating_sub(context);
        let end = std::cmp::min(total_chars, match_pos + context);
        let slice: String = text.chars().skip(start).take(end - start).collect();

        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(20, 10))
            .show(ui, |ui| {
                ui.add(egui::Label::new(&slice).wrap().selectable(true));
            });
    } else {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(20, 10))
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(&text).wrap().selectable(true));
                    });
            });
    }
}

fn render_page_image(ui: &mut egui::Ui, doc: &Arc<Mutex<Box<dyn Document>>>, page: usize, scale: f32) {
    let page_data = doc.lock().render_page(page, scale);

    match page_data {
        Some(p) => {
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [p.width as usize, p.height as usize],
                &p.rgba,
            );
            let texture = ui.ctx().load_texture(
                format!("doc_page_{}", page),
                color_image,
                egui::TextureOptions::default(),
            );
            ui.vertical_centered(|ui| {
                ui.image((texture.id(), egui::Vec2::new(p.width as f32, p.height as f32)));
            });
        }
        None => {
            ui.label("Image not available.");
        }
    }
}

fn render_paged_image(ui: &mut egui::Ui, doc: &Arc<Mutex<Box<dyn Document>>>, rs: &mut ReadingState) {
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            render_page_image(ui, doc, rs.page, rs.scale);
        });
}

fn render_continuous_images(ui: &mut egui::Ui, doc: &Arc<Mutex<Box<dyn Document>>>, page: &mut usize, scale: f32, total: usize, initial_scroll: Option<f32>, out_scroll_y: &mut f32, id_salt: &str) {
    let id = ui.make_persistent_id(id_salt);
    let spacing = 12.0;

    if total == 0 { return; }
    *page = (*page).min(total - 1);

    // Build page layout: (width, height, y_offset) for every page
    let mut layouts: Vec<(f32, f32, f32)> = Vec::with_capacity(total);
    {
        let d = doc.lock();
        let mut y = 0.0;
        for i in 0..total {
            let (w, h) = d.page_size(i, scale).unwrap_or((800.0, 1000.0));
            layouts.push((w, h, y));
            y += h + spacing;
        }
    }

    // Use previous frame's scroll position for visibility inside the closure.
    // When a mode transition sets initial_scroll, use that instead so the
    // first frame shows the correct pages (no flicker).
    let mut prev_scroll_y = *out_scroll_y;
    if let Some(off) = initial_scroll {
        prev_scroll_y = off;
    }
    let approx_vph = ui.available_size().y;

    let mut sa = egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false; 2]);
    if let Some(off) = initial_scroll {
        sa = sa.vertical_scroll_offset(off);
    }

    // show() returns ScrollAreaOutput with authoritative state.
    let output = sa.show(ui, |ui| {
        let approx_bottom = prev_scroll_y + approx_vph;

        for (i, &(_pw, ph, py)) in layouts.iter().enumerate() {
            // Fast visibility check using previous frame's scroll position.
            // Slightly stale is fine — one frame lag is invisible to the user.
            if py + ph >= prev_scroll_y && py <= approx_bottom {
                render_page_image(ui, doc, i, scale);
            } else {
                ui.allocate_exact_size(egui::vec2(ui.available_width(), ph), egui::Sense::hover());
            }
            if i + 1 < total {
                ui.add_space(spacing);
            }
        }
    });

    // Use authoritative state from ScrollAreaOutput — no get_persisted needed
    let scroll_y = output.state.offset.y;
    let viewport_h = output.inner_rect.height();
    *out_scroll_y = scroll_y;

    // Determine current page by largest visible ratio
    let viewport_bottom = scroll_y + viewport_h;
    let mut best_page = *page;
    let mut best_ratio = 0.0;
    for (i, &(_pw, ph, py)) in layouts.iter().enumerate() {
        let visible_top = py.max(scroll_y);
        let visible_bottom = (py + ph).min(viewport_bottom);
        if visible_top < visible_bottom {
            let ratio = (visible_bottom - visible_top) / ph;
            if ratio > best_ratio {
                best_ratio = ratio;
                best_page = i;
            }
        }
    }
    *page = best_page;
}

pub fn render_auto(ui: &mut egui::Ui, document: &Arc<Mutex<Box<dyn Document>>>, aut: &mut AutoState, ctx: egui::Context) {
    let supports_image = document.lock().supports_image();

    if supports_image {
        if aut.playing {
            let dt = ui.input(|i| i.unstable_dt);
            let page_count = document.lock().page_count();
            aut.progress += dt * aut.speed * 0.5;
            if aut.progress >= page_count as f32 && page_count > 0 {
                aut.progress -= page_count as f32;
            }
            ctx.request_repaint();
        }

        let current_page = aut.progress as usize;
        let total = document.lock().page_count();
        let target_y = if total > 0 && current_page < total {
            let d = document.lock();
            let mut y = 0.0;
            for i in 0..current_page {
                let (_, h) = d.page_size(i, 1.0).unwrap_or((800.0, 1000.0));
                y += h + 12.0;
            }
            Some(y)
        } else {
            None
        };

        let mut auto_page = current_page;
        let mut dummy_scroll = 0.0;
        render_continuous_images(ui, document, &mut auto_page, 1.0, total, target_y, &mut dummy_scroll, "pdf_scroll_auto");
    } else {
        if aut.playing {
            let dt = ui.input(|i| i.unstable_dt);
            aut.progress += dt * aut.speed * 0.05;
            if aut.progress >= 1.0 {
                aut.progress -= 1.0;
            }
            ctx.request_repaint();
        }

        render_auto_text(ui, document, aut);
    }
}

fn render_auto_text(ui: &mut egui::Ui, doc: &Arc<Mutex<Box<dyn Document>>>, aut: &mut AutoState) {
    let full_text = doc.lock().page_text(0);
    let total_chars = full_text.chars().count();
    if total_chars == 0 {
        return;
    }

    let avail_w = ui.available_width().max(100.0);
    let avail_h = ui.available_height().max(100.0);

    let chars_per_line = (avail_w / 12.0) as usize;
    let lines_per_view = (avail_h / 22.0) as usize;
    let viewport_chars = (chars_per_line * lines_per_view).max(200);

    let start_char = (aut.progress * total_chars as f32) as usize;
    let end_char = std::cmp::min(total_chars, start_char + viewport_chars);

    let slice: String = full_text.chars().skip(start_char).take(end_char - start_char).collect();

    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(20, 10))
        .show(ui, |ui| {
            ui.add(egui::Label::new(&slice).wrap().selectable(true));
        });
}

pub fn render_annotate(ui: &mut egui::Ui, document: &Arc<Mutex<Box<dyn Document>>>, an: &mut AnnotateState) {
    let supports_image = document.lock().supports_image();

    if supports_image {
        let total = document.lock().page_count();
        let mut annotate_page = an.page;
        let mut dummy_scroll = 0.0;
        render_continuous_images(ui, document, &mut annotate_page, 1.0, total, Some(0.0), &mut dummy_scroll, "pdf_scroll_annotate");
    } else {
        let text = document.lock().page_text(0);
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(20, 10))
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(&text).wrap().selectable(true));
                    });
            });
    }
}
