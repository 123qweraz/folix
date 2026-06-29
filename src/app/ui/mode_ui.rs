use crate::app::engines::Document;
use crate::app::engines::TextWordPosition;
use crate::app::core::mode_system::{ReadingState, ReadingLayout, Bookmark, AutoState, AnnotateState, SelectionState};
use std::sync::Arc;
use std::collections::HashMap;
use parking_lot::Mutex;

pub fn render_document(
    ui: &mut egui::Ui,
    document: &Arc<Mutex<Box<dyn Document>>>,
    page: &mut usize,
    scale: &mut f32,
    reading_layout: &mut ReadingLayout,
    reading: &mut ReadingState,
    auto: Option<&mut AutoState>,
    annotate: Option<&mut AnnotateState>,
    ctx: Option<egui::Context>,
) {
    let supports_image = document.lock().supports_image();

    if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F)) {
        reading.show_sidebar = true;
        reading.search.show_search = true;
    }

    if let Some(aut) = auto {
        if aut.playing {
            let dt = ui.input(|i| i.unstable_dt);
            if supports_image {
                let total = document.lock().page_count();
                let max_page = total.saturating_sub(1);
                match reading_layout {
                    ReadingLayout::Paged => {
                        aut.progress += dt * aut.speed * 0.3;
                        let advance = aut.progress as usize;
                        if advance > 0 {
                            *page = (*page + advance).min(max_page);
                            aut.progress -= advance as f32;
                            if *page >= max_page && total > 0 {
                                aut.playing = false;
                                aut.progress = 0.0;
                            }
                        }
                    }
                    ReadingLayout::Scroll => {
                        reading.scroll_offset_y += dt * 200.0 * aut.speed;
                    }
                }
            } else {
                aut.progress += dt * aut.speed * 0.05;
                if aut.progress >= 1.0 {
                    aut.progress -= 1.0;
                }
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        }
    }

    if supports_image {
        match *reading_layout {
            ReadingLayout::Paged => {
                render_paged(ui, document, *page, *scale, &mut reading.selection, annotate);
            }
            ReadingLayout::Scroll => {
                let total = document.lock().page_count();
                render_scroll(ui, document, page, *scale, total, &mut reading.scroll_offset_y, &mut reading.selection, annotate);
            }
        }
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

fn render_paged(
    ui: &mut egui::Ui,
    doc: &Arc<Mutex<Box<dyn Document>>>,
    page: usize,
    scale: f32,
    selection: &mut SelectionState,
    annotate: Option<&mut AnnotateState>,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            let all_words = {
                let d = doc.lock();
                let mut m = HashMap::new();
                m.insert(page, d.page_text_positions(page));
                m
            };
            render_image_page(ui, doc, page, scale, &all_words, selection, annotate);
        });
}

fn render_scroll(
    ui: &mut egui::Ui,
    doc: &Arc<Mutex<Box<dyn Document>>>,
    page: &mut usize,
    scale: f32,
    total: usize,
    out_scroll_y: &mut f32,
    selection: &mut SelectionState,
    mut annotate: Option<&mut AnnotateState>,
) {
    let id = ui.make_persistent_id("pdf_scroll_reading");
    let spacing = 12.0;

    if total == 0 { return; }
    *page = (*page).min(total - 1);

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

    let initial_scroll = if *out_scroll_y == 0.0 && *page > 0 {
        let target = (*page).min(total - 1);
        layout_peek(&layouts, target)
    } else {
        None
    };
    let mut prev_scroll_y = *out_scroll_y;
    if let Some(off) = initial_scroll {
        prev_scroll_y = off;
    }
    let approx_vph = ui.available_size().y;

    let all_words: HashMap<usize, Vec<TextWordPosition>> = {
        let d = doc.lock();
        layouts.iter().enumerate()
            .filter(|(_, &(_, ph, py))| py + ph >= prev_scroll_y && py <= prev_scroll_y + approx_vph)
            .map(|(i, _)| (i, d.page_text_positions(i)))
            .collect()
    };

    let mut sa = egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false; 2]);
    if let Some(off) = initial_scroll {
        sa = sa.vertical_scroll_offset(off);
    }

    let output = sa.show(ui, |ui| {
        let approx_bottom = prev_scroll_y + approx_vph;

        for (i, &(_pw, ph, py)) in layouts.iter().enumerate() {
            if py + ph >= prev_scroll_y && py <= approx_bottom {
                let an = annotate.as_mut().map(|r| &mut **r);
                render_image_page(ui, doc, i, scale, &all_words, selection, an);
            } else {
                ui.allocate_exact_size(egui::vec2(ui.available_width(), ph), egui::Sense::hover());
            }
            if i + 1 < total {
                ui.add_space(spacing);
            }
        }
    });

    let scroll_y = output.state.offset.y;
    let viewport_h = output.inner_rect.height();
    *out_scroll_y = scroll_y;

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

fn layout_peek(layouts: &[(f32, f32, f32)], idx: usize) -> Option<f32> {
    layouts.get(idx).map(|&(_, _, py)| py)
}

fn render_image_page(
    ui: &mut egui::Ui,
    doc: &Arc<Mutex<Box<dyn Document>>>,
    page_idx: usize,
    scale: f32,
    all_words: &HashMap<usize, Vec<TextWordPosition>>,
    selection: &mut SelectionState,
    mut _annotate: Option<&mut AnnotateState>,
) {
    // Acquire texture (from GPU cache or render + upload)
    let cached_tex = doc.lock().get_texture_handle(page_idx, scale);
    let (tex_id, image_size) = match cached_tex {
        Some((id, [w, h])) => (id, egui::Vec2::new(w as f32, h as f32)),
        None => {
            let page_data = doc.lock().render_page(page_idx, scale);
            let Some(p) = page_data else {
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 1000.0), egui::Sense::hover());
                return;
            };
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [p.width as usize, p.height as usize],
                &p.rgba,
            );
            let tex = ui.ctx().load_texture(
                format!("doc_page_{}", page_idx),
                color_image,
                egui::TextureOptions::default(),
            );
            let size = egui::Vec2::new(p.width as f32, p.height as f32);
            doc.lock().set_texture_handle(page_idx, scale, tex.clone());
            (tex.id(), size)
        }
    };

    // Layout
    let avail_w = ui.available_width();
    let x_off = ((avail_w - image_size.x) * 0.5).max(0.0);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(avail_w, image_size.y),
        egui::Sense::click_and_drag(),
    );
    let image_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(x_off, 0.0),
        image_size,
    );

    ui.painter().image(
        tex_id,
        image_rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );

    // --- Text selection (always active) ---
    let words = all_words.get(&page_idx);

    // Render highlight for selected words
    if let Some(words) = words {
        if !selection.selected_word_indices.is_empty() && selection.page == page_idx {
            for &idx in &selection.selected_word_indices {
                if let Some(w) = words.get(idx) {
                    let highlight_rect = egui::Rect::from_min_max(
                        egui::pos2(image_rect.left() + w.x0 * scale, image_rect.top() + w.y0 * scale),
                        egui::pos2(image_rect.left() + w.x1 * scale, image_rect.top() + w.y1 * scale),
                    );
                    ui.painter().rect_filled(
                        highlight_rect,
                        0.0,
                        egui::Color32::from_rgba_premultiplied(100, 150, 255, 100),
                    );
                }
            }
        }
    }

    // Drag to select
    if response.drag_started() {
        if let Some(mouse_pos) = response.interact_pointer_pos() {
            let rel_x = (mouse_pos.x - image_rect.left()) / scale;
            let rel_y = (mouse_pos.y - image_rect.top()) / scale;
            selection.selecting = true;
            selection.anchor = Some((rel_x, rel_y));
            selection.focus = Some((rel_x, rel_y));
            selection.page = page_idx;
            if let Some(words) = words {
                selection.selected_word_indices = find_words_in_range(words, rel_x, rel_y, rel_x, rel_y);
            }
        }
    }

    if selection.selecting && selection.page == page_idx && response.dragged() {
        if let Some(mouse_pos) = response.interact_pointer_pos() {
            let rel_x = (mouse_pos.x - image_rect.left()) / scale;
            let rel_y = (mouse_pos.y - image_rect.top()) / scale;
            selection.focus = Some((rel_x, rel_y));
            if let (Some(anchor), Some(focus)) = (selection.anchor, selection.focus) {
                if let Some(words) = words {
                    selection.selected_word_indices = find_words_in_range(
                        words,
                        anchor.0.min(focus.0),
                        anchor.1.min(focus.1),
                        anchor.0.max(focus.0),
                        anchor.1.max(focus.1),
                    );
                }
            }
        }
    }

    if response.drag_stopped() {
        selection.selecting = false;
    }

    // Drag indicator line
    if selection.selecting && selection.page == page_idx {
        if let (Some(anchor), Some(focus)) = (selection.anchor, selection.focus) {
            let from = egui::pos2(
                image_rect.left() + anchor.0 * scale,
                image_rect.top() + anchor.1 * scale,
            );
            let to = egui::pos2(
                image_rect.left() + focus.0 * scale,
                image_rect.top() + focus.1 * scale,
            );
            ui.painter().line_segment(
                [from, to],
                egui::Stroke::new(2.0, egui::Color32::from_rgba_premultiplied(100, 150, 255, 200)),
            );
        }
    }

    // --- Right-click context menu ---
    response.context_menu(|ui| {
        if !selection.selected_word_indices.is_empty() && selection.page == page_idx {
            if let Some(words) = all_words.get(&page_idx) {
                if ui.button("📋 Copy").clicked() {
                    let selected_text: String = selection.selected_word_indices
                        .iter()
                        .filter_map(|&i| words.get(i))
                        .map(|w| w.text.as_str())
                        .collect::<Vec<&str>>()
                        .join(" ");
                    ui.ctx().copy_text(selected_text);
                    ui.close_menu();
                }
            }
        }
    });
}

fn find_words_in_range(
    words: &[TextWordPosition],
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
) -> Vec<usize> {
    words
        .iter()
        .enumerate()
        .filter(|(_, w)| {
            w.x0 < x1 && w.x1 > x0 && w.y0 < y1 && w.y1 > y0
        })
        .map(|(i, _)| i)
        .collect()
}

// --- Sidebar (unchanged) ---

pub fn render_sidebar(ui: &mut egui::Ui, document: &Arc<Mutex<Box<dyn Document>>>, page: &mut usize, rs: &mut ReadingState, total: usize) {
    ui.heading("Sidebar");
    ui.separator();

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
                            let selected = *page == entry.page_index;
                            if ui.selectable_label(selected, &entry.label).clicked() {
                                let target = entry.page_index.min(total.saturating_sub(1));
                                *page = target;
                                rs.scroll_offset_y = 0.0;
                            }
                        }
                    });
            }
        });

    ui.separator();

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

    egui::CollapsingHeader::new("🔖 Bookmarks")
        .default_open(true)
        .show(ui, |ui| {
            let mut remove_idx: Option<usize> = None;
            for (idx, bm) in rs.bookmarks.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.selectable_label(false, format!("{} (p.{})", bm.label, bm.page + 1)).clicked() {
                        *page = bm.page;
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
                let label = format!("Page {}", *page + 1);
                rs.bookmarks.push(Bookmark { page: *page, label });
            }
        });
}
