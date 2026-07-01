use crate::app::engines::{Document, ContentBlock, TextWordPosition};
use crate::app::core::mode_system::{ReadingState, ReadingLayout, Bookmark, AutoState, AnnotateState, SelectionState, Annotation, AnnotationTool};
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
    dark_mode: bool,
    image_cache: &mut HashMap<String, egui::TextureHandle>,
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
        let highlights = &reading.search.page_highlights;
        match *reading_layout {
            ReadingLayout::Paged => {
                render_paged(ui, document, *page, *scale, &mut reading.selection, annotate, dark_mode, highlights);
            }
            ReadingLayout::Scroll => {
                let total = document.lock().page_count();
                render_scroll(ui, document, page, *scale, total, &mut reading.scroll_offset_y, &mut reading.selection, annotate, dark_mode, highlights);
            }
        }
    } else {
        let blocks = document.lock().content_blocks(0);

        // Handle TOC scroll target for non-image docs
        let mut init_scroll_y: Option<f32> = None;
        if let Some(char_off) = reading.scroll_to_char_offset.take() {
            let font_size = 16.0;
            let line_h = font_size * 1.5;
            let text_width = ui.available_width().max(100.0).max(100.0) - 40.0;
            let cpl = (text_width / (font_size * 0.55)).max(10.0);
            let lines = char_off as f32 / cpl;
            init_scroll_y = Some(lines * line_h);
        }

        let mut sa = egui::ScrollArea::vertical()
            .auto_shrink([false; 2]);
        if let Some(y) = init_scroll_y {
            sa = sa.vertical_scroll_offset(y);
        }

        // Enable multi-widget text selection so selection can span paragraphs
        ui.style_mut().interaction.multi_widget_text_select = true;

        sa.show(ui, |ui| {
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(20, 10))
                .show(ui, |ui| {
                    for (idx, block) in blocks.iter().enumerate() {
                        match block {
                            ContentBlock::Text(text) => {
                                ui.add(
                                    egui::Label::new(text.as_str())
                                        .wrap()
                                        .selectable(true),
                                );
                            }
                            ContentBlock::Image(img) => {
                                let key = format!("epub_img_{}", idx);
                                let texture = image_cache.entry(key.clone()).or_insert_with(|| {
                                    let decoded = match image::load_from_memory(&img.raw_bytes) {
                                        Ok(d) => d.into_rgba8(),
                                        Err(_) => {
                                            return ui.ctx().load_texture(
                                                &key,
                                                egui::ColorImage::new([1, 1], egui::Color32::RED),
                                                egui::TextureOptions::default(),
                                            );
                                        }
                                    };
                                    let (w, h) = decoded.dimensions();
                                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                        [w as usize, h as usize],
                                        decoded.as_raw(),
                                    );
                                    ui.ctx().load_texture(
                                        &key,
                                        color_image,
                                        egui::TextureOptions::default(),
                                    )
                                });
                                let aspect = img.width as f32 / img.height as f32;
                                let max_w = ui.available_width().min(600.0);
                                let h = max_w / aspect;
                                ui.add_sized(
                                    egui::vec2(max_w, h + 8.0),
                                    egui::Image::new((texture.id(), egui::vec2(max_w, h))),
                                );
                            }
                        }
                    }
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
    dark_mode: bool,
    highlights: &std::collections::HashMap<usize, Vec<usize>>,
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
            render_image_page(ui, doc, page, scale, &all_words, selection, annotate, dark_mode, highlights);
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
    dark_mode: bool,
    highlights: &std::collections::HashMap<usize, Vec<usize>>,
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
                render_image_page(ui, doc, i, scale, &all_words, selection, an, dark_mode, highlights);
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
    mut annotate: Option<&mut AnnotateState>,
    _dark_mode: bool,
    highlights: &std::collections::HashMap<usize, Vec<usize>>,
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

    let words = all_words.get(&page_idx);

    // --- Interaction (selection in all modes, tool-specific overrides) ---
    let tool = annotate.as_ref().map(|a| a.tool.clone());

    // Render text selection overlay (always, regardless of tool)
    if let Some(words_data) = words {
        if !selection.selected_word_indices.is_empty() && selection.page == page_idx {
            for &idx in &selection.selected_word_indices {
                if let Some(w) = words_data.get(idx) {
                    let r = egui::Rect::from_min_max(
                        egui::pos2(image_rect.left() + w.x0 * scale, image_rect.top() + w.y0 * scale),
                        egui::pos2(image_rect.left() + w.x1 * scale, image_rect.top() + w.y1 * scale),
                    );
                    ui.painter().rect_filled(
                        r, 0.0,
                        egui::Color32::from_rgba_premultiplied(100, 150, 255, 100),
                    );
                }
            }
        }
    }

    // Dispatch interaction based on active tool
    if tool == Some(AnnotationTool::Pen) {
        // --- Pen: freehand drawing (monopolizes drag events) ---
        if response.drag_started() {
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                let rx = (mouse_pos.x - image_rect.left()) / scale;
                let ry = (mouse_pos.y - image_rect.top()) / scale;
                if let Some(ann) = annotate.as_mut() {
                    ann.stroke_points.clear();
                    ann.stroke_points.push([rx, ry]);
                }
            }
        }

        if let Some(ann) = annotate.as_mut() {
            if response.dragged() {
                if let Some(mouse_pos) = response.interact_pointer_pos() {
                    let rx = (mouse_pos.x - image_rect.left()) / scale;
                    let ry = (mouse_pos.y - image_rect.top()) / scale;
                    ann.stroke_points.push([rx, ry]);
                }
            }

            if response.drag_stopped() {
                if !ann.stroke_points.is_empty() {
                    let pts = ann.stroke_points.clone();
                    let data = serde_json::to_string(&pts).unwrap_or_default();
                    ann.annotations.push(Annotation {
                        id: uuid::Uuid::new_v4().to_string(),
                        doc_id: String::new(),
                        kind: AnnotationTool::Pen,
                        page: page_idx,
                        rect: [0.0, 0.0, 0.0, 0.0],
                        note: Some(data),
                        color: ann.current_color,
                    });
                    ann.stroke_points.clear();
                }
            }

            // Draw in-progress stroke
            if !ann.stroke_points.is_empty() {
                let points: Vec<egui::Pos2> = ann.stroke_points.iter().map(|&[x, y]| {
                    egui::pos2(image_rect.left() + x * scale, image_rect.top() + y * scale)
                }).collect();
                if points.len() > 1 {
                    for w in points.windows(2) {
                        ui.painter().line_segment(
                            [w[0], w[1]],
                            egui::Stroke::new(3.0, egui::Color32::from_rgba_premultiplied(255, 100, 50, 200)),
                        );
                    }
                }
            }
        }
    } else if tool == Some(AnnotationTool::Eraser) {
        // --- Eraser: click-to-delete annotation (monopolizes drag events) ---
        let mut erase_pos: Option<(f32, f32)> = None;
        if response.drag_started() {
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                let rx = (mouse_pos.x - image_rect.left()) / scale;
                let ry = (mouse_pos.y - image_rect.top()) / scale;
                if let Some(ann) = annotate.as_mut() {
                    ann.selection_anchor = Some((rx, ry));
                }
            }
        }
        if response.drag_stopped() {
            if let Some(ann) = annotate.as_mut() {
                if let (Some(mouse_pos), Some(anchor)) = (response.interact_pointer_pos(), ann.selection_anchor) {
                    let rx = (mouse_pos.x - image_rect.left()) / scale;
                    let ry = (mouse_pos.y - image_rect.top()) / scale;
                    let dx = rx - anchor.0;
                    let dy = ry - anchor.1;
                    if (dx * dx + dy * dy).sqrt() * scale < 8.0 {
                        erase_pos = Some((rx, ry));
                    }
                }
                ann.selection_anchor = None;
            }
        }
        if let Some((rx, ry)) = erase_pos {
            if let Some(ann) = annotate.as_mut() {
                let hit = ann.annotations.iter().position(|a| {
                    if a.page != page_idx { return false; }
                    match a.kind {
                        AnnotationTool::Highlight => {
                            rx >= a.rect[0] && rx <= a.rect[2] && ry >= a.rect[1] && ry <= a.rect[3]
                        }
                        AnnotationTool::Pen => {
                            if let Some(data) = &a.note {
                                if let Ok(pts) = serde_json::from_str::<Vec<[f32; 2]>>(data) {
                                    let th = 15.0 / scale;
                                    pts.iter().any(|&[x, y]| {
                                        let ddx = x - rx;
                                        let ddy = y - ry;
                                        (ddx * ddx + ddy * ddy).sqrt() < th
                                    })
                                } else { false }
                            } else { false }
                        }
                        AnnotationTool::Note => {
                            let ddx = a.rect[0] - rx;
                            let ddy = a.rect[1] - ry;
                            (ddx * ddx + ddy * ddy).sqrt() < 25.0 / scale
                        }
                        AnnotationTool::Eraser => false,
                    }
                });
                if let Some(idx) = hit {
                    ann.annotations.remove(idx);
                }
            }
        }
    } else {
        // --- Text selection (Highlight / None / Note tools) ---
        let shift_held = ui.input(|i| i.modifiers.shift);

        // Double-click → select word
        if response.double_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let rx = (pos.x - image_rect.left()) / scale;
                let ry = (pos.y - image_rect.top()) / scale;
                if let Some(words_data) = words {
                    if let Some(idx) = find_word_at(words_data, rx, ry) {
                        let w = &words_data[idx];
                        selection.selected_word_indices = vec![idx];
                        selection.anchor = Some((w.x0, w.y0));
                        selection.focus = Some((w.x1, w.y1));
                        selection.page = page_idx;
                        selection.selecting = false;
                    }
                }
            }
        }

        // Click (no shift) on word → select just that word; on blank → clear selection
        if !shift_held && response.clicked() && !response.double_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let rx = (pos.x - image_rect.left()) / scale;
                let ry = (pos.y - image_rect.top()) / scale;
                if let Some(words_data) = words {
                    if let Some(idx) = find_word_at(words_data, rx, ry) {
                        let w = &words_data[idx];
                        selection.selected_word_indices = vec![idx];
                        selection.anchor = Some((w.x0, w.y0));
                        selection.focus = Some((w.x1, w.y1));
                        selection.page = page_idx;
                    } else {
                        selection.selected_word_indices.clear();
                        selection.anchor = None;
                        selection.focus = None;
                    }
                    selection.selecting = false;
                }
            }
        }

        // Shift+click → toggle word in/out of selection (multi-select)
        if shift_held && response.clicked() && !response.double_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let rx = (pos.x - image_rect.left()) / scale;
                let ry = (pos.y - image_rect.top()) / scale;
                if let Some(words_data) = words {
                    if let Some(idx) = find_word_at(words_data, rx, ry) {
                        if selection.page == page_idx {
                            if let Some(pos) = selection.selected_word_indices.iter().position(|&i| i == idx) {
                                selection.selected_word_indices.remove(pos);
                            } else {
                                selection.selected_word_indices.push(idx);
                            }
                        } else {
                            selection.selected_word_indices = vec![idx];
                            selection.page = page_idx;
                        }
                    }
                    selection.selecting = false;
                }
            }
        }

        // Drag selection (normal drag replaces, Ctrl+drag adds to selection)
        if !shift_held && !response.double_clicked() && response.drag_started() {
            let ctrl_held = ui.input(|i| i.modifiers.ctrl);
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                let rx = (mouse_pos.x - image_rect.left()) / scale;
                let ry = (mouse_pos.y - image_rect.top()) / scale;
                selection.selecting = true;
                selection.anchor = Some((rx, ry));
                selection.focus = Some((rx, ry));
                selection.page = page_idx;
                if !ctrl_held {
                    if let Some(words_data) = words {
                        selection.selected_word_indices = find_words_in_range(words_data, rx, ry, rx, ry);
                    }
                }
                // Ctrl+drag: keep existing selection, just begin tracking
            }
        }

        if selection.selecting && selection.page == page_idx && response.dragged() {
            let ctrl_held = ui.input(|i| i.modifiers.ctrl);
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                let rx = (mouse_pos.x - image_rect.left()) / scale;
                let ry = (mouse_pos.y - image_rect.top()) / scale;
                selection.focus = Some((rx, ry));
                if let (Some(anchor), Some(focus)) = (selection.anchor, selection.focus) {
                    if let Some(words_data) = words {
                        let range_words = find_words_in_range(
                            words_data,
                            anchor.0.min(focus.0),
                            anchor.1.min(focus.1),
                            anchor.0.max(focus.0),
                            anchor.1.max(focus.1),
                        );
                        if ctrl_held {
                            for &idx in &range_words {
                                if !selection.selected_word_indices.contains(&idx) {
                                    selection.selected_word_indices.push(idx);
                                }
                            }
                        } else {
                            selection.selected_word_indices = range_words;
                        }
                    }
                }
            }
        }

        if response.drag_stopped() {
            selection.selecting = false;
        }

        // Drag preview line
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

        // Note tool: click on existing highlight to edit note
        if tool == Some(AnnotationTool::Note) && !shift_held && response.clicked() && !response.double_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let rx = (pos.x - image_rect.left()) / scale;
                let ry = (pos.y - image_rect.top()) / scale;
                if let Some(ann) = annotate.as_mut() {
                    let hit = ann.annotations.iter().position(|a| {
                        a.page == page_idx && a.kind == AnnotationTool::Highlight &&
                        rx >= a.rect[0] && rx <= a.rect[2] && ry >= a.rect[1] && ry <= a.rect[3]
                    });
                    if let Some(idx) = hit {
                        ann.editing_note_id = Some(ann.annotations[idx].id.clone());
                        ann.note_text_buffer = ann.annotations[idx].note.clone().unwrap_or_default();
                    }
                }
            }
        }
    }

    // Note edit popup (shown when editing_note_id is set, regardless of tool)
    if let Some(ann) = annotate.as_mut() {
        if let Some(ref edit_id) = ann.editing_note_id.clone() {
            let mut keep = true;
            egui::Window::new("Edit Note")
                .open(&mut keep)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.label("Note text:");
                    ui.add_space(4.0);
                    ui.add(egui::TextEdit::multiline(&mut ann.note_text_buffer)
                        .desired_width(200.0)
                        .desired_rows(4));
                    ui.add_space(8.0);
                    if ui.button("Save").clicked() {
                        if let Some(a) = ann.annotations.iter_mut().find(|a| a.id == *edit_id) {
                            a.note = if ann.note_text_buffer.is_empty() {
                                None
                            } else {
                                Some(ann.note_text_buffer.clone())
                            };
                        }
                        ann.editing_note_id = None;
                        ann.note_text_buffer.clear();
                    }
                });
            if !keep {
                ann.editing_note_id = None;
                ann.note_text_buffer.clear();
            }
        }
    }

    // --- Render all existing annotations (always, when in DeepReading mode) ---
    if let Some(ann) = annotate.as_ref() {
        for ann_item in &ann.annotations {
            if ann_item.page != page_idx { continue; }
            match ann_item.kind {
                AnnotationTool::Highlight => {
                    let [x0, y0, x1, y1] = ann_item.rect;
                    let c = ann_item.color;
                    let r = egui::Rect::from_min_max(
                        egui::pos2(image_rect.left() + x0 * scale, image_rect.top() + y0 * scale),
                        egui::pos2(image_rect.left() + x1 * scale, image_rect.top() + y1 * scale),
                    );
                    ui.painter().rect_filled(r, 0.0, egui::Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]));
                }
                AnnotationTool::Pen => {
                    if let Some(data) = &ann_item.note {
                        if let Ok(pts) = serde_json::from_str::<Vec<[f32; 2]>>(data) {
                            let points: Vec<egui::Pos2> = pts.iter().map(|&[x, y]| {
                                egui::pos2(image_rect.left() + x * scale, image_rect.top() + y * scale)
                            }).collect();
                            for w in points.windows(2) {
                                ui.painter().line_segment(
                                    [w[0], w[1]],
                                    egui::Stroke::new(3.0, egui::Color32::from_rgba_premultiplied(255, 100, 50, 200)),
                                );
                            }
                        }
                    }
                }
                AnnotationTool::Note => {
                    let cx = image_rect.left() + ann_item.rect[0] * scale;
                    let cy = image_rect.top() + ann_item.rect[1] * scale;
                    let size = 12.0;
                    ui.painter().circle_filled(
                        egui::pos2(cx, cy), size,
                        egui::Color32::from_rgba_premultiplied(255, 200, 50, 200),
                    );
                    ui.painter().text(
                        egui::pos2(cx, cy),
                        egui::Align2::CENTER_CENTER, "📌",
                        egui::FontId::proportional(14.0),
                        egui::Color32::WHITE,
                    );
                    if let Some(text) = &ann_item.note {
                        if !text.is_empty() {
                            let line_count = text.lines().count() as f32;
                            let text_rect = egui::Rect::from_min_size(
                                egui::pos2(cx + size + 4.0, cy - 8.0),
                                egui::vec2(180.0, 16.0 * line_count + 8.0),
                            ).intersect(egui::Rect::from_min_max(
                                egui::pos2(image_rect.left(), image_rect.top()),
                                egui::pos2(image_rect.right(), image_rect.bottom()),
                            ));
                            ui.painter().rect_filled(
                                text_rect, 4.0,
                                egui::Color32::from_rgba_premultiplied(255, 240, 180, 220),
                            );
                            ui.painter().rect_stroke(
                                text_rect, 4.0,
                                egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(200, 160, 40, 200)),
                                egui::StrokeKind::Inside,
                            );
                            ui.painter().text(
                                egui::pos2(text_rect.left() + 4.0, text_rect.top() + 4.0),
                                egui::Align2::LEFT_TOP, text,
                                egui::FontId::proportional(12.0),
                                egui::Color32::BLACK,
                            );
                        }
                    }
                }
                AnnotationTool::Eraser => {}
            }
        }
    }

    // --- Render search highlights ---
    if let Some(words) = words {
        if let Some(page_highlights) = highlights.get(&page_idx) {
            for &idx in page_highlights {
                if let Some(w) = words.get(idx) {
                    let r = egui::Rect::from_min_max(
                        egui::pos2(image_rect.left() + w.x0 * scale, image_rect.top() + w.y0 * scale),
                        egui::pos2(image_rect.left() + w.x1 * scale, image_rect.top() + w.y1 * scale),
                    );
                    ui.painter().rect_filled(
                        r, 0.0,
                        egui::Color32::from_rgba_premultiplied(255, 150, 50, 120),
                    );
                }
            }
        }
    }

    // --- Right-click context menu (just Copy) ---
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

fn find_word_at(words: &[TextWordPosition], x: f32, y: f32) -> Option<usize> {
    words.iter().enumerate().find(|(_, w)| x >= w.x0 && x <= w.x1 && y >= w.y0 && y <= w.y1).map(|(i, _)| i)
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
            let is_image = document.lock().supports_image();
            if toc.is_empty() {
                ui.label("No table of contents");
            } else {
                egui::ScrollArea::vertical()
                    .max_height(f32::INFINITY)
                    .show(ui, |ui| {
                        for entry in &toc {
                            let selected = if is_image { *page == entry.page_index } else { false };
                            if ui.selectable_label(selected, &entry.label).clicked() {
                                if is_image {
                                    let target = entry.page_index.min(total.saturating_sub(1));
                                    *page = target;
                                    rs.scroll_offset_y = 0.0;
                                } else {
                                    rs.scroll_to_char_offset = Some(entry.page_index);
                                }
                            }
                        }
                    });
            }
        });

    ui.separator();

    egui::CollapsingHeader::new("🔍 Search")
        .default_open(true)
        .show(ui, |ui| {
            let prev_query = rs.search.query.clone();
            ui.add(egui::TextEdit::singleline(&mut rs.search.query)
                .hint_text("Search text...")
                .desired_width(f32::INFINITY));

            if rs.search.query != prev_query {
                rs.search.matches.clear();
                rs.search.page_highlights.clear();
                rs.search.current_match = 0;
                if !rs.search.query.is_empty() {
                    let lower_query = rs.search.query.to_lowercase();
                    let is_image = document.lock().supports_image();
                    if is_image {
                        let total = document.lock().page_count();
                        for p in 0..total {
                            let words = document.lock().page_text_positions(p);
                            let page_matches: Vec<usize> = words.iter().enumerate()
                                .filter(|(_, w)| w.text.to_lowercase().contains(&lower_query))
                                .map(|(i, _)| i)
                                .collect();
                            if !page_matches.is_empty() {
                                rs.search.matches.push(p);
                                rs.search.page_highlights.insert(p, page_matches);
                            }
                        }
                    } else {
                        let full_text = document.lock().page_text(0);
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
            }

            let is_image = document.lock().supports_image();
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
                    if is_image {
                        if let Some(&p) = rs.search.matches.get(rs.search.current_match) {
                            if p != *page {
                                *page = p;
                                rs.scroll_offset_y = 0.0;
                            }
                        }
                    } else if let Some(&co) = rs.search.matches.get(rs.search.current_match) {
                        rs.scroll_to_char_offset = Some(co);
                    }
                }
                if ui.add_enabled(enabled, egui::Button::new("▼")).clicked() {
                    rs.search.current_match = if current + 1 >= total_matches { 0 } else { current + 1 };
                    if is_image {
                        if let Some(&p) = rs.search.matches.get(rs.search.current_match) {
                            if p != *page {
                                *page = p;
                                rs.scroll_offset_y = 0.0;
                            }
                        }
                    } else if let Some(&co) = rs.search.matches.get(rs.search.current_match) {
                        rs.scroll_to_char_offset = Some(co);
                    }
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
