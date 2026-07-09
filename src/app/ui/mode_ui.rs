use crate::app::engines::{DocumentHandle, ContentBlock, TextWordPosition};
use crate::app::core::mode_system::{ReadingState, ReadingLayout, FitMode, ViewRotation, Bookmark, AutoState, AnnotateState, SelectionState, Annotation, AnnotationTool, Vocabulary, SidebarSection, MoYuState};
use std::sync::Arc;
use std::collections::HashMap;
use parking_lot::Mutex;


pub fn render_document(
    ui: &mut egui::Ui,
    document: &Arc<Mutex<DocumentHandle>>,
    page: &mut usize,
    scale: &mut f32,
    reading_layout: &mut ReadingLayout,
    fit_mode: &mut FitMode,
    view_rotation: &mut ViewRotation,
    reading: &mut ReadingState,
    auto: Option<&mut AutoState>,
    annotate: Option<&mut AnnotateState>,
    ctx: Option<egui::Context>,
    dark_mode: bool,
    image_cache: &mut HashMap<String, egui::TextureHandle>,
) {
    let is_fixed = document.lock().is_fixed();

    // Calculate fit scale for fixed layouts
    if *fit_mode != FitMode::Free && is_fixed {
        if let Some(fixed) = document.lock().as_fixed() {
            if let Some((w, h)) = fixed.page_size(*page, 1.0) {
                let (disp_w, disp_h) = match *view_rotation {
                    ViewRotation::Deg0 | ViewRotation::Deg180 => (w, h),
                    ViewRotation::Deg90 | ViewRotation::Deg270 => (h, w),
                };
                let (avail_w, avail_h) = (ui.available_width(), ui.available_height());
                if disp_w > 0.0 && disp_h > 0.0 {
                    let new_scale = match *fit_mode {
                        FitMode::FitWidth => (avail_w - 20.0) / disp_w,
                        FitMode::FitPage => {
                            let sw = (avail_w - 20.0) / disp_w;
                            let sh = (avail_h - 20.0) / disp_h;
                            sw.min(sh)
                        }
                        FitMode::Free => *scale,
                    };
                    *scale = (new_scale.max(0.1).min(10.0) * 100.0).round() / 100.0;
                }
            }
        }
    }

    let doc = document.lock();
    let is_fixed = doc.is_fixed();

    if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F)) {
        reading.show_sidebar = true;
        reading.search.show_search = true;
    }

    if let Some(aut) = auto {
        if aut.playing {
            let dt = ui.input(|i| i.unstable_dt);
            if is_fixed {
                if let Some(fixed) = doc.as_fixed() {
                    let total = fixed.page_count();
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
                }
            } else {
                // Reflow (EPUB/TXT) auto-scroll: drive velocity like the ▼ button does.
                reading.scroll_velocity = 200.0 * aut.speed;
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        }
    }

    if is_fixed {
        drop(doc);
        let highlights = &reading.search.page_highlights;

        // Preload next page's GPU texture before rendering,
        // so render_image_page hits the texture cache when a new
        // page scrolls into view.
        if let Some(fixed) = document.lock().as_fixed() {
            let total = fixed.page_count();
            if *page + 1 < total {
                if fixed.get_texture_handle(*page + 1, *scale).is_none() {
                    if let Some(p) = fixed.render_page(*page + 1, *scale) {
                        let ci = egui::ColorImage::from_rgba_unmultiplied(
                            [p.width as usize, p.height as usize],
                            &p.rgba,
                        );
                        let tex = ui.ctx().load_texture(
                            format!("doc_page_{}", *page + 1),
                            ci,
                            egui::TextureOptions::default(),
                        );
                        fixed.set_texture_handle(*page + 1, *scale, tex);
                    }
                }
            }
            if *page > 0 { fixed.render_page(*page - 1, *scale); }
        }

        match *reading_layout {
            ReadingLayout::Paged => {
                render_paged(ui, document, *page, *scale, *view_rotation, &mut reading.selection, annotate, dark_mode, highlights);
            }
            ReadingLayout::Scroll => {
                let total = document.lock().as_fixed().map(|f| f.page_count()).unwrap_or(0);
                if reading.scroll_velocity != 0.0 {
                    let dt = ui.input(|i| i.unstable_dt);
                    reading.scroll_offset_y =
                        (reading.scroll_offset_y + reading.scroll_velocity * dt).max(0.0);
                }
                let target = reading.scroll_offset_y;
                render_scroll(ui, document, page, *scale, *view_rotation, total, &mut reading.scroll_offset_y, &mut reading.selection, annotate, dark_mode, highlights);
                if reading.scroll_velocity != 0.0 {
                    reading.scroll_offset_y = target;
                }
                reading.scroll_velocity = 0.0;
            }
        }
    } else {
        drop(doc);

        if reading.chapter_cache.is_empty() {
            let doc_handle = document.lock();
            if let Some(reflow) = doc_handle.as_reflow() {
                for ci in 0..reflow.chapter_count() {
                    reading.chapter_cache.push(Some(reflow.load_chapter(ci)));
                }
            }
        }

        let mut sa = egui::ScrollArea::vertical()
            .id_salt("reflow_stream")
            .auto_shrink([false; 2]);

        if reading.scroll_velocity != 0.0 {
            let dt = ui.input(|i| i.unstable_dt);
            reading.scroll_offset_y =
                (reading.scroll_offset_y + reading.scroll_velocity * dt).max(0.0);
            sa = sa.vertical_scroll_offset(reading.scroll_offset_y);
        }

        let font_size = 16.0 * *scale;

        let output;
        if reading.show_line_numbers {
            // ---- Virtual scrolling with per-source-line rendering ----
            let avail_w = ui.available_width().max(1.0);
            let gutter_w = 65.0;
            let text_avail_w = (avail_w - gutter_w).max(1.0);
            let cpl = (text_avail_w / (font_size * 0.55)).floor().max(1.0) as usize;
            let line_h = font_size * 1.4;

            struct LR {
                line_no: usize,
                ci: usize,
                bi: usize,
                it: u8, // 0=sep, 1=text, 2=image
                text: String,
                height: f32,
            }

            let mut rows: Vec<LR> = Vec::new();
            let mut global_line: usize = 0;

            for (ci, chapter_opt) in reading.chapter_cache.iter().enumerate() {
                let chapter = match chapter_opt.as_ref() {
                    Some(ch) => ch,
                    None => continue,
                };
                if ci > 0 {
                    rows.push(LR { line_no: 0, ci, bi: 0, it: 0, text: String::new(), height: 12.0 });
                }
                for (bi, block) in chapter.blocks.iter().enumerate() {
                    match block {
                        ContentBlock::Text(text) => {
                            for src_line in text.split('\n') {
                                let lno = global_line + 1;
                                let nc = src_line.chars().count().max(1) as f32;
                                let vlines = (nc / cpl as f32).ceil().max(1.0);
                                rows.push(LR {
                                    line_no: lno, ci, bi,
                                    it: 1,
                                    text: src_line.to_string(),
                                    height: vlines * line_h,
                                });
                                global_line += 1;
                            }
                        }
                        ContentBlock::Image(img) => {
                            let max_w = avail_w.min(600.0);
                            let aspect = img.width as f32 / img.height as f32;
                            let h = max_w / aspect;
                            rows.push(LR {
                                line_no: global_line + 1, ci, bi,
                                it: 2,
                                text: String::new(),
                                height: h + 8.0,
                            });
                            global_line += 1;
                        }
                    }
                }
            }

            // Pre-compute row start Y positions for binary-search culling
            let mut row_starts = Vec::with_capacity(rows.len());
            let mut acc_y = 0.0;
            for r in &rows {
                row_starts.push(acc_y);
                acc_y += r.height;
            }
            let total_height = acc_y;
            reading.total_height = total_height;
            let rows_ref = &rows;
            let row_starts_ref = &row_starts;
            let chapter_cache_ref = &reading.chapter_cache;

            let text_color = ui.style().visuals.text_color();

            output = sa.show_viewport(ui, |ui, viewport| {
                ui.set_height(total_height);

                let first = row_starts_ref.partition_point(|&y| y < viewport.top());
                let last = row_starts_ref.partition_point(|&y| y < viewport.bottom());
                let last = last.min(rows_ref.len());

                for i in first..last {
                    let row = &rows_ref[i];
                    let y = row_starts_ref[i];
                    let rect = egui::Rect::from_min_size(
                        egui::pos2(0.0, y),
                        egui::vec2(ui.available_width(), row.height),
                    );
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
                        match row.it {
                            0 => {
                                ui.separator();
                            }
                            1 => {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!("{:>6}│ ", row.line_no))
                                            .size(font_size)
                                            .color(egui::Color32::GRAY),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&row.text).size(font_size).color(text_color),
                                        ).wrap(),
                                    );
                                });
                            }
                            _ => {
                                let key = format!("epub_img_{}_{}", row.ci, row.bi);
                                let texture = image_cache.entry(key.clone()).or_insert_with(|| {
                                    let ch = chapter_cache_ref[row.ci].as_ref().unwrap();
                                    let img = match &ch.blocks[row.bi] {
                                        ContentBlock::Image(img) => img,
                                        _ => unreachable!(),
                                    };
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
                                    let (native_w, native_h) = decoded.dimensions();
                                    let aspect = native_w as f32 / native_h as f32;
                                    let display_w = (ui.available_width().min(600.0)).ceil() as u32;
                                    let display_h = (display_w as f32 / aspect).ceil() as u32;
                                    let resized = if display_w < native_w {
                                        image::imageops::resize(
                                            &decoded,
                                            display_w.max(1),
                                            display_h.max(1),
                                            image::imageops::FilterType::Lanczos3,
                                        )
                                    } else {
                                        decoded
                                    };
                                    let (rw, rh) = resized.dimensions();
                                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                        [rw as usize, rh as usize],
                                        resized.as_raw(),
                                    );
                                    ui.ctx().load_texture(&key, color_image, egui::TextureOptions::default())
                                });
                                let max_w = avail_w.min(600.0);
                                let ch = chapter_cache_ref[row.ci].as_ref().unwrap();
                                let img = match &ch.blocks[row.bi] {
                                    ContentBlock::Image(img) => img,
                                    _ => unreachable!(),
                                };
                                let aspect = img.width as f32 / img.height as f32;
                                let h = max_w / aspect;
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!("{:>6}│ ", row.line_no))
                                            .size(font_size)
                                            .color(egui::Color32::GRAY),
                                    );
                                    ui.add_sized(
                                        egui::vec2(max_w, h + 8.0),
                                        egui::Image::new((texture.id(), egui::vec2(max_w, h))),
                                    );
                                });
                            }
                        }
                    });
                }
            });
        } else {
            // Original block-level rendering (no line numbers)
            output = sa.show(ui, |ui| {
                let mut job_text = String::new();
                let mut job_sections: Vec<egui::text::LayoutSection> = Vec::new();

                let flush_job = |ui: &mut egui::Ui, jt: &mut String, js: &mut Vec<egui::text::LayoutSection>| {
                    if !jt.is_empty() {
                        let job = egui::text::LayoutJob {
                            text: std::mem::take(jt),
                            sections: std::mem::take(js),
                            break_on_newline: true,
                            ..Default::default()
                        };
                        ui.add(egui::Label::new(job));
                    }
                };

                for (ci, chapter_opt) in reading.chapter_cache.iter().enumerate() {
                    let chapter = match chapter_opt.as_ref() {
                        Some(ch) => ch,
                        None => continue,
                    };

                    if ci > 0 {
                        flush_job(ui, &mut job_text, &mut job_sections);
                        ui.separator();
                    }

                    for (bi, block) in chapter.blocks.iter().enumerate() {
                        match block {
                            ContentBlock::Text(text) => {
                                flush_job(ui, &mut job_text, &mut job_sections);
                                let text_len = text.chars().count();
                                if text_len > 0 {
                                    let label = egui::Label::new(
                                        egui::RichText::new(text.as_str()).size(font_size),
                                    )
                                    .wrap();
                                    let resp = ui.add(label);
                                    let label_rect = resp.rect;

                                    if let Some(ann) = annotate.as_ref() {
                                        for h in &ann.annotations {
                                            if h.kind != AnnotationTool::Highlight {
                                                continue;
                                            }
                                            if let Some((h_ch, h_blk, h_cs, h_ce)) =
                                                h.reflow_range
                                            {
                                                if h_ch == ci && h_blk == bi {
                                                    let h_start =
                                                        h_cs.max(0).min(text_len);
                                                    let h_end =
                                                        h_ce.max(0).min(text_len);
                                                    if h_start < h_end {
                                                        let overlap_text: String = text
                                                            .chars()
                                                            .skip(h_start)
                                                            .take(h_end - h_start)
                                                            .collect();
                                                        let before_text: String = text
                                                            .chars()
                                                            .skip(0)
                                                            .take(h_start)
                                                            .collect();
                                                        let before_w = ui.fonts(|f| {
                                                            f.layout_no_wrap(
                                                                before_text,
                                                                egui::FontId::proportional(
                                                                    font_size,
                                                                ),
                                                                egui::Color32::WHITE,
                                                            )
                                                            .rect
                                                            .width()
                                                        });
                                                        let overlap_w = ui.fonts(|f| {
                                                            f.layout_no_wrap(
                                                                overlap_text,
                                                                egui::FontId::proportional(
                                                                    font_size,
                                                                ),
                                                                egui::Color32::WHITE,
                                                            )
                                                            .rect
                                                            .width()
                                                        });
                                                        let c = h.color;
                                                        let hl_rect = egui::Rect::from_min_size(
                                                            egui::pos2(
                                                                label_rect.left() + before_w,
                                                                label_rect.top(),
                                                            ),
                                                            egui::vec2(
                                                                overlap_w,
                                                                label_rect.height(),
                                                            ),
                                                        );
                                                        ui.painter().rect_filled(
                                                            hl_rect,
                                                            0.0,
                                                            egui::Color32::from_rgba_premultiplied(
                                                                c[0], c[1], c[2], c[3],
                                                            ),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    let sel = &mut reading.selection;
                                    if sel.selecting && sel.char_anchor.is_some() {
                                        let (a_ch, a_blk, a_pos) = sel.char_anchor.unwrap();
                                        let (f_ch, f_blk, f_pos) =
                                            sel.char_focus.unwrap_or((a_ch, a_blk, a_pos));
                                        if a_ch == ci && a_blk == bi
                                            || f_ch == ci && f_blk == bi
                                        {
                                            let s_start = (if a_ch == ci && a_blk == bi { a_pos } else { 0 }).max(0).min(text_len);
                                            let s_end = (if f_ch == ci && f_blk == bi { f_pos } else { text_len }).max(0).min(text_len);
                                            let s_low = s_start.min(s_end);
                                            let s_high = s_start.max(s_end);
                                            if s_low < s_high {
                                                let before_text: String = text
                                                    .chars()
                                                    .skip(0)
                                                    .take(s_low)
                                                    .collect();
                                                let sel_text: String = text
                                                    .chars()
                                                    .skip(s_low)
                                                    .take(s_high - s_low)
                                                    .collect();
                                                let before_w = ui.fonts(|f| {
                                                    f.layout_no_wrap(
                                                        before_text,
                                                        egui::FontId::proportional(font_size),
                                                        egui::Color32::WHITE,
                                                    )
                                                    .rect
                                                    .width()
                                                });
                                                let sel_w = ui.fonts(|f| {
                                                    f.layout_no_wrap(
                                                        sel_text,
                                                        egui::FontId::proportional(font_size),
                                                        egui::Color32::WHITE,
                                                    )
                                                    .rect
                                                    .width()
                                                });
                                                let sel_rect = egui::Rect::from_min_size(
                                                    egui::pos2(
                                                        label_rect.left() + before_w,
                                                        label_rect.top(),
                                                    ),
                                                    egui::vec2(sel_w, label_rect.height()),
                                                );
                                                ui.painter().rect_filled(
                                                    sel_rect,
                                                    0.0,
                                                    egui::Color32::from_rgba_premultiplied(
                                                        100, 150, 255, 100,
                                                    ),
                                                );
                                            }
                                        }
                                    }

                                    if resp.clicked() {
                                        if let Some(mouse_pos) = resp.interact_pointer_pos() {
                                            let local_x = mouse_pos.x - label_rect.left();
                                            let ratio = (local_x
                                                / label_rect.width().max(1.0))
                                            .clamp(0.0, 1.0);
                                            let approx_char =
                                                (ratio * text_len as f32) as usize;
                                            sel.char_anchor = Some((ci, bi, approx_char));
                                            sel.char_focus = Some((ci, bi, approx_char));
                                            sel.selected_text = String::new();
                                            sel.selected_word_indices.clear();
                                            sel.selecting = true;
                                        }
                                    }

                                    if resp.dragged() && sel.selecting {
                                        if let Some(mouse_pos) = resp.interact_pointer_pos() {
                                            let local_x = mouse_pos.x - label_rect.left();
                                            let ratio = (local_x
                                                / label_rect.width().max(1.0))
                                            .clamp(0.0, 1.0);
                                            let approx_char =
                                                (ratio * text_len as f32) as usize;
                                            sel.char_focus = Some((ci, bi, approx_char));
                                        }
                                    }

                                    if resp.drag_stopped() {
                                        sel.selecting = false;
                                    }

                                    resp.context_menu(|ui| {
                                        if let (Some(anchor), Some(focus)) =
                                            (sel.char_anchor, sel.char_focus)
                                        {
                                            let (a_ch, a_blk, a_pos) = anchor;
                                            let (f_ch, f_blk, f_pos) = focus;
                                            if a_ch == ci && a_blk == bi
                                                || f_ch == ci && f_blk == bi
                                            {
                                                let local_a = if a_ch == ci && a_blk == bi { a_pos } else { 0 };
                                                let local_f = if f_ch == ci && f_blk == bi { f_pos } else { text_len };
                                                let s_start = local_a.min(local_f).max(0).min(text_len);
                                                let s_end = local_a.max(local_f).max(0).min(text_len);
                                                if s_start < s_end {
                                                    let sel_text: String = text
                                                        .chars()
                                                        .skip(s_start)
                                                        .take(s_end - s_start)
                                                        .collect();
                                                    if ui.button("Copy").clicked() {
                                                        ui.ctx().copy_text(sel_text.clone());
                                                        ui.close_menu();
                                                    }
                                                    if ui.button("Add to Vocabulary")
                                                        .clicked()
                                                    {
                                                        sel.pending_vocab =
                                                            Some(sel_text.clone());
                                                        ui.close_menu();
                                                    }
                                                    if ui.button("Save Sentence")
                                                        .clicked()
                                                    {
                                                        sel.pending_sentence =
                                                            Some(sel_text);
                                                        ui.close_menu();
                                                    }
                                                }
                                            }
                                        }
                                    });
                                }
                            }
                            ContentBlock::Image(img) => {
                                flush_job(ui, &mut job_text, &mut job_sections);
                                let key = format!("epub_img_{}_{}", ci, bi);
                                let texture = image_cache.entry(key.clone()).or_insert_with(|| {
                                    let decoded = match image::load_from_memory(&img.raw_bytes)
                                    {
                                        Ok(d) => d.into_rgba8(),
                                        Err(_) => {
                                            return ui.ctx().load_texture(
                                                &key,
                                                egui::ColorImage::new(
                                                    [1, 1],
                                                    egui::Color32::RED,
                                                ),
                                                egui::TextureOptions::default(),
                                            );
                                        }
                                    };
                                    let (native_w, native_h) = decoded.dimensions();
                                    let aspect = native_w as f32 / native_h as f32;
                                    let display_w =
                                        (ui.available_width().min(600.0)).ceil() as u32;
                                    let display_h = (display_w as f32 / aspect).ceil() as u32;
                                    let resized = if display_w < native_w {
                                        image::imageops::resize(
                                            &decoded,
                                            display_w.max(1),
                                            display_h.max(1),
                                            image::imageops::FilterType::Lanczos3,
                                        )
                                    } else {
                                        decoded
                                    };
                                    let (rw, rh) = resized.dimensions();
                                    let color_image =
                                        egui::ColorImage::from_rgba_unmultiplied(
                                            [rw as usize, rh as usize],
                                            resized.as_raw(),
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
                                    egui::Image::new((
                                        texture.id(),
                                        egui::vec2(max_w, h),
                                    )),
                                );
                            }
                        }
                    }
                }
                flush_job(ui, &mut job_text, &mut job_sections);
            });
        }

        if reading.scroll_velocity == 0.0 {
            reading.scroll_offset_y = output.state.offset.y;
        }
        reading.scroll_velocity = 0.0;
    }
}

fn render_paged(
    ui: &mut egui::Ui,
    doc: &Arc<Mutex<DocumentHandle>>,
    page: usize,
    scale: f32,
    view_rotation: ViewRotation,
    selection: &mut SelectionState,
    annotate: Option<&mut AnnotateState>,
    dark_mode: bool,
    highlights: &std::collections::HashMap<usize, Vec<usize>>,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            let need_words = annotate.is_some()
                || !selection.selected_word_indices.is_empty();
            let all_words = if need_words {
                let d = doc.lock();
                if let Some(fixed) = d.as_fixed() {
                    let mut m = HashMap::new();
                    m.insert(page, fixed.page_text_positions(page));
                    m
                } else {
                    HashMap::new()
                }
            } else {
                HashMap::new()
            };
            render_image_page(ui, doc, page, scale, view_rotation, &all_words, selection, annotate, dark_mode, highlights);
        });
}

fn render_scroll(
    ui: &mut egui::Ui,
    doc: &Arc<Mutex<DocumentHandle>>,
    page: &mut usize,
    scale: f32,
    view_rotation: ViewRotation,
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
        if let Some(fixed) = d.as_fixed() {
            let mut y = 0.0;
            for i in 0..total {
                let (w, h) = fixed.page_size(i, scale).unwrap_or((800.0, 1000.0));
                let (disp_w, disp_h) = match view_rotation {
                    ViewRotation::Deg0 | ViewRotation::Deg180 => (w, h),
                    ViewRotation::Deg90 | ViewRotation::Deg270 => (h, w),
                };
                layouts.push((disp_w, disp_h, y));
                y += disp_h + spacing;
            }
        }
    }

    let prev_scroll_y = *out_scroll_y;
    let approx_vph = ui.available_size().y;

    // Page jump: first render with page > 0
    let jump_y = if prev_scroll_y == 0.0 && *page > 0 {
        layout_peek(&layouts, (*page).min(total - 1))
    } else {
        None
    };
    let scroll_target = jump_y.unwrap_or(prev_scroll_y);

    // Only extract text positions when selection or annotation is active.
    // During auto-play (Light mode) there's no selection, so skip to avoid
    // the expensive MuPDF text extraction on each page's first appearance.
    let need_words = annotate.is_some()
        || !selection.selected_word_indices.is_empty();
    let all_words: HashMap<usize, Vec<TextWordPosition>> = if need_words {
        let d = doc.lock();
        if let Some(fixed) = d.as_fixed() {
            layouts.iter().enumerate()
                .filter(|(_, &(_, ph, py))| py + ph >= scroll_target && py <= scroll_target + approx_vph)
                .map(|(i, _)| (i, fixed.page_text_positions(i)))
                .collect()
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    };

    let mut sa = egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false; 2]);
    sa = sa.vertical_scroll_offset(scroll_target);

    let output = sa.show(ui, |ui| {
        let approx_bottom = scroll_target + approx_vph;

        for (i, &(_pw, ph, py)) in layouts.iter().enumerate() {
            if py + ph >= scroll_target && py <= approx_bottom {
                let an = annotate.as_mut().map(|r| &mut **r);
                render_image_page(ui, doc, i, scale, view_rotation, &all_words, selection, an, dark_mode, highlights);
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
    doc: &Arc<Mutex<DocumentHandle>>,
    page_idx: usize,
    scale: f32,
    view_rotation: ViewRotation,
    all_words: &HashMap<usize, Vec<TextWordPosition>>,
    selection: &mut SelectionState,
    mut annotate: Option<&mut AnnotateState>,
    _dark_mode: bool,
    highlights: &std::collections::HashMap<usize, Vec<usize>>,
) {
    // Acquire texture (from GPU cache or render + upload)
    let (tex_id, tex_size) = {
        let d = doc.lock();
        let fixed = d.as_fixed().unwrap();
        let cached_tex = fixed.get_texture_handle(page_idx, scale);
        match cached_tex {
            Some((id, [w, h])) => (id, egui::Vec2::new(w as f32, h as f32)),
            None => {
                let page_data = fixed.render_page(page_idx, scale);
                drop(d);
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
                let d2 = doc.lock();
                if let Some(fixed) = d2.as_fixed() {
                    fixed.set_texture_handle(page_idx, scale, tex.clone());
                }
                (tex.id(), size)
            }
        }
    };

    let page_nat_w = tex_size.x;
    let page_nat_h = tex_size.y;

    // Display size accounts for rotation
    let (disp_w, disp_h) = match view_rotation {
        ViewRotation::Deg0 | ViewRotation::Deg180 => (page_nat_w, page_nat_h),
        ViewRotation::Deg90 | ViewRotation::Deg270 => (page_nat_h, page_nat_w),
    };
    let display_size = egui::Vec2::new(disp_w, disp_h);

    // Layout
    let avail_w = ui.available_width();
    let x_off = ((avail_w - display_size.x) * 0.5).max(0.0);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(avail_w, display_size.y),
        egui::Sense::click_and_drag(),
    );
    let image_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(x_off, 0.0),
        display_size,
    );

    // Draw rotated image using a mesh with rotated UVs
    if view_rotation == ViewRotation::Deg0 {
        ui.painter().image(
            tex_id,
            image_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        let (tl_uv, tr_uv, br_uv, bl_uv) = rotated_uvs(view_rotation);
        let mut mesh = egui::epaint::Mesh::with_texture(tex_id);
        let c = egui::Color32::WHITE;
        mesh.vertices.push(egui::epaint::Vertex { pos: image_rect.left_top(), uv: tl_uv, color: c });
        mesh.vertices.push(egui::epaint::Vertex { pos: image_rect.right_top(), uv: tr_uv, color: c });
        mesh.vertices.push(egui::epaint::Vertex { pos: image_rect.right_bottom(), uv: br_uv, color: c });
        mesh.vertices.push(egui::epaint::Vertex { pos: image_rect.left_bottom(), uv: bl_uv, color: c });
        mesh.indices.extend_from_slice(&[0, 1, 2, 2, 3, 0]);
        ui.painter().add(egui::Shape::mesh(mesh));
    }

    // Helper: screen position → document coordinates (page space)
    let screen_to_doc = |mx: f32, my: f32| -> (f32, f32) {
        let nx = (mx - image_rect.left()) / display_size.x;
        let ny = (my - image_rect.top()) / display_size.y;
        let (doc_nx, doc_ny) = match view_rotation {
            ViewRotation::Deg0 => (nx, ny),
            ViewRotation::Deg90 => (1.0 - ny, nx),
            ViewRotation::Deg180 => (1.0 - nx, 1.0 - ny),
            ViewRotation::Deg270 => (ny, 1.0 - nx),
        };
        (doc_nx * page_nat_w, doc_ny * page_nat_h)
    };

    // Helper: document coordinates → screen position
    let doc_to_screen = |dx: f32, dy: f32| -> (f32, f32) {
        let nx = dx / page_nat_w;
        let ny = dy / page_nat_h;
        let (sx, sy) = match view_rotation {
            ViewRotation::Deg0 => (nx, ny),
            ViewRotation::Deg90 => (ny, 1.0 - nx),
            ViewRotation::Deg180 => (1.0 - nx, 1.0 - ny),
            ViewRotation::Deg270 => (1.0 - ny, nx),
        };
        (image_rect.left() + sx * display_size.x, image_rect.top() + sy * display_size.y)
    };

    let words = all_words.get(&page_idx);

    // --- Interaction (selection in all modes, tool-specific overrides) ---
    let tool = annotate.as_ref().map(|a| a.tool.clone());

    // Render text selection overlay (always, regardless of tool)
    if let Some(words_data) = words {
        if !selection.selected_word_indices.is_empty() && selection.page == page_idx {
            for &idx in &selection.selected_word_indices {
                if let Some(w) = words_data.get(idx) {
                    let (sx0, sy0) = doc_to_screen(w.x0, w.y0);
                    let (sx1, sy1) = doc_to_screen(w.x1, w.y1);
                    let r = egui::Rect::from_min_max(
                        egui::pos2(sx0, sy0), egui::pos2(sx1, sy1),
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
        if response.drag_started() {
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                let (rx, ry) = screen_to_doc(mouse_pos.x, mouse_pos.y);
                if let Some(ann) = annotate.as_mut() {
                    ann.stroke_points.clear();
                    ann.stroke_points.push([rx, ry]);
                }
            }
        }

        if let Some(ann) = annotate.as_mut() {
            if response.dragged() {
                if let Some(mouse_pos) = response.interact_pointer_pos() {
                    let (rx, ry) = screen_to_doc(mouse_pos.x, mouse_pos.y);
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
                        rect: [0.0; 4],
                        note: Some(data),
                        color: ann.current_color,
                        reflow_range: None,
                    });
                    ann.dirty = true;
                    ann.stroke_points.clear();
                }
            }

            if !ann.stroke_points.is_empty() {
                let points: Vec<egui::Pos2> = ann.stroke_points.iter().map(|&[x, y]| {
                    let (sx, sy) = doc_to_screen(x, y);
                    egui::pos2(sx, sy)
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
        let mut erase_pos: Option<(f32, f32)> = None;
        if response.drag_started() {
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                let (rx, ry) = screen_to_doc(mouse_pos.x, mouse_pos.y);
                if let Some(ann) = annotate.as_mut() {
                    ann.selection_anchor = Some((rx, ry));
                }
            }
        }
        if response.drag_stopped() {
            if let Some(ann) = annotate.as_mut() {
                if let (Some(mouse_pos), Some(anchor)) = (response.interact_pointer_pos(), ann.selection_anchor) {
                    let (rx, ry) = screen_to_doc(mouse_pos.x, mouse_pos.y);
                    let dx = rx - anchor.0;
                    let dy = ry - anchor.1;
                    if (dx * dx + dy * dy).sqrt() < 8.0 {
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
                                    let th = 15.0;
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
                            (ddx * ddx + ddy * ddy).sqrt() < 25.0
                        }
                        AnnotationTool::Eraser => false,
                    }
                });
                if let Some(idx) = hit {
                    ann.annotations.remove(idx);
                    ann.dirty = true;
                }
            }
        }
    } else {
        // --- Text selection (Highlight / None / Note tools) ---
        let shift_held = ui.input(|i| i.modifiers.shift);

        if response.double_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (rx, ry) = screen_to_doc(pos.x, pos.y);
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

        if !shift_held && response.clicked() && !response.double_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (rx, ry) = screen_to_doc(pos.x, pos.y);
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

        if shift_held && response.clicked() && !response.double_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (rx, ry) = screen_to_doc(pos.x, pos.y);
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

        if !shift_held && !response.double_clicked() && response.drag_started() {
            let ctrl_held = ui.input(|i| i.modifiers.ctrl);
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                let (rx, ry) = screen_to_doc(mouse_pos.x, mouse_pos.y);
                selection.selecting = true;
                selection.anchor = Some((rx, ry));
                selection.focus = Some((rx, ry));
                selection.page = page_idx;
                if !ctrl_held {
                    if let Some(words_data) = words {
                        selection.selected_word_indices = find_words_in_range(words_data, rx, ry, rx, ry);
                    }
                }
            }
        }

        if selection.selecting && selection.page == page_idx && response.dragged() {
            let ctrl_held = ui.input(|i| i.modifiers.ctrl);
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                let (rx, ry) = screen_to_doc(mouse_pos.x, mouse_pos.y);
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

        if selection.selecting && selection.page == page_idx {
            if let (Some(anchor), Some(focus)) = (selection.anchor, selection.focus) {
                let (from_x, from_y) = doc_to_screen(anchor.0, anchor.1);
                let (to_x, to_y) = doc_to_screen(focus.0, focus.1);
                ui.painter().line_segment(
                    [egui::pos2(from_x, from_y), egui::pos2(to_x, to_y)],
                    egui::Stroke::new(2.0, egui::Color32::from_rgba_premultiplied(100, 150, 255, 200)),
                );
            }
        }

        if tool == Some(AnnotationTool::Note) && !shift_held && response.clicked() && !response.double_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (rx, ry) = screen_to_doc(pos.x, pos.y);
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
                            ann.dirty = true;
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

    if let Some(ann) = annotate.as_ref() {
        for ann_item in &ann.annotations {
            if ann_item.page != page_idx { continue; }
            match ann_item.kind {
                AnnotationTool::Highlight => {
                    let [x0, y0, x1, y1] = ann_item.rect;
                    let c = ann_item.color;
                    let (sx0, sy0) = doc_to_screen(x0, y0);
                    let (sx1, sy1) = doc_to_screen(x1, y1);
                    let r = egui::Rect::from_min_max(
                        egui::pos2(sx0, sy0), egui::pos2(sx1, sy1),
                    );
                    ui.painter().rect_filled(r, 0.0, egui::Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]));
                }
                AnnotationTool::Pen => {
                    if let Some(data) = &ann_item.note {
                        if let Ok(pts) = serde_json::from_str::<Vec<[f32; 2]>>(data) {
                            let points: Vec<egui::Pos2> = pts.iter().map(|&[x, y]| {
                                let (sx, sy) = doc_to_screen(x, y);
                                egui::pos2(sx, sy)
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
                    let (cx, cy) = doc_to_screen(ann_item.rect[0], ann_item.rect[1]);
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

    if let Some(words) = words {
        if let Some(page_highlights) = highlights.get(&page_idx) {
            for &idx in page_highlights {
                if let Some(w) = words.get(idx) {
                    let (sx0, sy0) = doc_to_screen(w.x0, w.y0);
                    let (sx1, sy1) = doc_to_screen(w.x1, w.y1);
                    let r = egui::Rect::from_min_max(
                        egui::pos2(sx0, sy0), egui::pos2(sx1, sy1),
                    );
                    ui.painter().rect_filled(
                        r, 0.0,
                        egui::Color32::from_rgba_premultiplied(255, 150, 50, 120),
                    );
                }
            }
        }
    }

    response.context_menu(|ui| {
        if !selection.selected_word_indices.is_empty() && selection.page == page_idx {
            if let Some(words) = all_words.get(&page_idx) {
                let selected_text: String = selection.selected_word_indices
                    .iter()
                    .filter_map(|&i| words.get(i))
                    .map(|w| w.text.as_str())
                    .collect::<Vec<&str>>()
                    .join(" ");
                if ui.button("📋 Copy").clicked() {
                    ui.ctx().copy_text(selected_text.clone());
                    ui.close_menu();
                }
                if ui.button("📝 Add to Vocabulary").clicked() {
                    selection.pending_vocab = Some(selected_text.clone());
                    ui.close_menu();
                }
                if ui.button("💬 Save Sentence").clicked() {
                    selection.pending_sentence = Some(selected_text);
                    ui.close_menu();
                }
            }
        }
    });
}

/// Split text at sentence and clause boundaries (。！？，；.!?,;\n).
/// Delimiters are consumed (not included in output chunks).
pub fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        if matches!(c, '。' | '！' | '？' | '，' | '；' | '.' | '!' | '?' | ',' | ';' | '\n') {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() && trimmed.len() > 1 {
                sentences.push(trimmed);
            }
            current.clear();
        } else {
            current.push(c);
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() && trimmed.len() > 1 {
        sentences.push(trimmed);
    }
    sentences
}

/// Extract sentences from text, splitting on Chinese/English punctuation.
pub fn extract_sentences(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![];
    }
    let mut sentences = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        current.push(c);
        if matches!(c, '。' | '！' | '？' | '，' | '；' | '：' | '.' | '!' | '?' | ',' | ';' | '\n') {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() && trimmed.len() > 1 {
                sentences.push(trimmed);
            }
            current.clear();
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() && trimmed.len() > 1 {
        sentences.push(trimmed);
    }
    sentences
}

/// Render the 摸鱼模式 UI — minimal single-line text with marquee for long sentences.
pub fn render_mo_yu_ui(
    ui: &mut egui::Ui,
    mo_yu: &mut MoYuState,
    document: &Option<Arc<Mutex<DocumentHandle>>>,
) {
    let bg = ui.style().visuals.window_fill();
    ui.painter().rect_filled(ui.max_rect(), 0.0, bg);

    let dt = ui.input(|i| i.unstable_dt);

    // Re-extract sentences if empty
    if mo_yu.sentences.is_empty() {
        if let Some(ref doc) = document {
            let doc = doc.lock();
            let text = if let Some(fixed) = doc.as_fixed() {
                fixed.page_text(mo_yu.page)
            } else if let Some(reflow) = doc.as_reflow() {
                let mut all = String::new();
                for i in 0..reflow.chapter_count() {
                    all.push_str(&reflow.chapter_text(i));
                    all.push('\n');
                }
                all
            } else {
                String::new()
            };
            drop(doc);
            let sentences = split_sentences(&text);
            if !sentences.is_empty() {
                mo_yu.sentences = sentences;
                mo_yu.sentence_idx = 0;
                mo_yu.timer = 0.0;
                mo_yu.scroll_x = 0.0;
            }
        }
    }

    let sentence = mo_yu.sentences.get(mo_yu.sentence_idx)
        .map(|s| s.as_str())
        .unwrap_or("");

    let text_color = ui.style().visuals.text_color();
    let font_id = egui::FontId::proportional(16.0);

    // Center vertically
    let av = ui.available_size();
    let content_h = 20.0;
    ui.add_space(((av.y - content_h) / 2.0).max(0.0));

    ui.horizontal(|ui| {
        // Drag handle — small grey block
        let (handle_rect, handle_resp) = ui.allocate_exact_size(
            egui::vec2(6.0, 14.0),
            egui::Sense::click(),
        );
        if handle_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if handle_resp.is_pointer_button_down_on() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        ui.painter().rect_filled(
            handle_rect,
            egui::CornerRadius::same(2),
            egui::Color32::from_gray(128),
        );

        // Measure text & available width
        let avail_w = ui.available_width().max(10.0);
        let text_w = ui.fonts(|f| f.layout_no_wrap(sentence.to_string(), font_id.clone(), text_color))
            .rect.width();

        if text_w <= avail_w {
            // Sentence fits — show normally
            ui.add(egui::Label::new(
                egui::RichText::new(sentence).size(16.0).color(text_color),
            ).selectable(false));
        } else {
            // Marquee: scroll text left continuously
            let (text_rect, _) = ui.allocate_exact_size(
                egui::vec2(avail_w, content_h),
                egui::Sense::hover(),
            );

            const SPEED: f32 = 50.0;
            let cycle = text_w + 60.0; // scroll + gap
            mo_yu.scroll_x = (mo_yu.scroll_x + dt * SPEED) % cycle;

            let paint_x = if mo_yu.scroll_x <= text_w {
                text_rect.left() - mo_yu.scroll_x
            } else {
                text_rect.left() + avail_w // gap, off-screen right
            };

            ui.painter().text(
                egui::pos2(paint_x, text_rect.center().y),
                egui::Align2::LEFT_CENTER,
                sentence,
                font_id,
                text_color,
            );
        }
    });

    // Advance timer when playing
    if mo_yu.playing && !mo_yu.sentences.is_empty() {
        let s = &mo_yu.sentences[mo_yu.sentence_idx];
        let base = (s.len() as f32 / 8.0).max(1.5) / mo_yu.speed;
        // For long sentences, give extra time for scrolling
        let text_w_est = s.len() as f32 * 9.0; // rough estimate
        let avail_w_est = 360.0;
        let extra = if text_w_est > avail_w_est { (text_w_est - avail_w_est) / 50.0 } else { 0.0 };
        let duration = (base + extra).min(15.0);

        mo_yu.timer += dt;
        if mo_yu.timer >= duration {
            mo_yu.timer = 0.0;
            mo_yu.scroll_x = 0.0;
            mo_yu.sentence_idx += 1;
            if mo_yu.sentence_idx >= mo_yu.sentences.len() {
                mo_yu.page += 1;
                mo_yu.sentences.clear();
                mo_yu.sentence_idx = 0;
            }
        }
    }

    if mo_yu.playing {
        ui.ctx().request_repaint();
    }
}

fn rotated_uvs(rotation: ViewRotation) -> (egui::Pos2, egui::Pos2, egui::Pos2, egui::Pos2) {
    match rotation {
        ViewRotation::Deg0 => (
            egui::pos2(0.0, 0.0),
            egui::pos2(1.0, 0.0),
            egui::pos2(1.0, 1.0),
            egui::pos2(0.0, 1.0),
        ),
        ViewRotation::Deg90 => (
            egui::pos2(1.0, 0.0),
            egui::pos2(1.0, 1.0),
            egui::pos2(0.0, 1.0),
            egui::pos2(0.0, 0.0),
        ),
        ViewRotation::Deg180 => (
            egui::pos2(1.0, 1.0),
            egui::pos2(0.0, 1.0),
            egui::pos2(0.0, 0.0),
            egui::pos2(1.0, 0.0),
        ),
        ViewRotation::Deg270 => (
            egui::pos2(0.0, 1.0),
            egui::pos2(0.0, 0.0),
            egui::pos2(1.0, 0.0),
            egui::pos2(1.0, 1.0),
        ),
    }
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

// --- Sidebar ---

pub fn render_sidebar(
    ui: &mut egui::Ui,
    document: &Arc<Mutex<DocumentHandle>>,
    page: &mut usize,
    rs: &mut ReadingState,
    lang: &str,
) {
    // Toggle buttons row
    ui.horizontal(|ui| {
        let sections = [
            (SidebarSection::TOC, "📖"),
            (SidebarSection::Search, "🔍"),
            (SidebarSection::Bookmarks, "🔖"),
            (SidebarSection::Vocab, "📝"),
            (SidebarSection::Sentences, "💬"),
        ];
        for &(section, icon) in &sections {
            let is_active = rs.sidebar_section == section;
            if ui.selectable_label(is_active, icon).clicked() {
                rs.sidebar_section = section;
            }
        }
    });
    ui.separator();

    let _total_pages = document.lock().toc_entries().len();

    match rs.sidebar_section {
        SidebarSection::TOC => {
            let toc = document.lock().toc_entries();
            if toc.is_empty() {
                ui.label(crate::app::i18n::tr(lang, "No table of contents"));
            } else {
                egui::ScrollArea::vertical()
                    .max_height(f32::INFINITY)
                    .show(ui, |ui| {
                        for entry in &toc {
                            let target_page = entry.page_index;
                            let selected = *page == target_page;
                            if ui.selectable_label(selected, &entry.label).clicked() {
                                rs.stream_jump_to = Some(target_page);
                                rs.stream_page_end = rs.stream_page_end.max(target_page);
                                *page = target_page;
                                rs.scroll_offset_y = 0.0;
                            }
                        }
                    });
            }
        }
        SidebarSection::Search => {
            let prev_query = rs.search.query.clone();
            ui.add(egui::TextEdit::singleline(&mut rs.search.query)
                .hint_text(crate::app::i18n::tr(lang, "Search text..."))
                .desired_width(f32::INFINITY));

            if rs.search.query != prev_query {
                rs.search.matches.clear();
                rs.search.page_highlights.clear();
                rs.search.current_match = 0;
                if !rs.search.query.is_empty() {
                    let lower_query = rs.search.query.to_lowercase();
                    let doc = document.lock();
                    if let Some(reflow) = doc.as_reflow() {
                        let count = reflow.chapter_count();
                        drop(doc);
                        for ci in 0..count {
                            let text = document.lock().as_reflow().unwrap().chapter_text(ci);
                            if text.to_lowercase().contains(&lower_query) {
                                rs.search.matches.push(ci);
                            }
                        }
                    } else {
                        drop(doc);
                        let total = document.lock().as_fixed().map(|f| f.page_count()).unwrap_or(0);
                        for p in 0..total {
                            let text = document.lock().as_fixed().unwrap().page_text(p);
                            if text.to_lowercase().contains(&lower_query) {
                                rs.search.matches.push(p);
                            }
                        }
                    }
                }
            }

            let total_matches = rs.search.matches.len();
            let current = rs.search.current_match;
            ui.horizontal(|ui| {
                if total_matches > 0 {
                    ui.label(format!("{}/{}", current + 1, total_matches));
                } else if !rs.search.query.is_empty() {
                    ui.label(crate::app::i18n::tr(lang, "0 matches"));
                }
                let enabled = total_matches > 0;
                if ui.add_enabled(enabled, egui::Button::new("▲")).clicked() {
                    rs.search.current_match = if current == 0 { total_matches - 1 } else { current - 1 };
                    if let Some(&m) = rs.search.matches.get(rs.search.current_match) {
                        let target = m;
                        if target != *page {
                            rs.stream_jump_to = Some(target);
                            rs.stream_page_end = rs.stream_page_end.max(target);
                            *page = target;
                            rs.scroll_offset_y = 0.0;
                        }
                    }
                }
                if ui.add_enabled(enabled, egui::Button::new("▼")).clicked() {
                    rs.search.current_match = if current + 1 >= total_matches { 0 } else { current + 1 };
                    if let Some(&m) = rs.search.matches.get(rs.search.current_match) {
                        let target = m;
                        if target != *page {
                            rs.stream_jump_to = Some(target);
                            rs.stream_page_end = rs.stream_page_end.max(target);
                            *page = target;
                            rs.scroll_offset_y = 0.0;
                        }
                    }
                }
            });
        }
        SidebarSection::Bookmarks => {
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
                rs.bookmarks_dirty = true;
            }

            if ui.button(crate::app::i18n::tr(lang, "+ Add Bookmark")).clicked() {
                let label = format!("{} {}", crate::app::i18n::tr(lang, "Page"), *page + 1);
                rs.bookmarks.push(Bookmark { page: *page, label });
                rs.bookmarks_dirty = true;
            }
        }
        SidebarSection::Vocab => {
            if ui.button(crate::app::i18n::tr(lang, "+ Add")).clicked() {
                rs.show_add_vocab_dialog = true;
                rs.add_vocab_text.clear();
            }
            let mut remove_idx: Option<usize> = None;
            for (idx, v) in rs.vocab.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.selectable_label(false, format!("{} (p.{})", v.word, v.page + 1)).clicked() {
                        *page = v.page;
                    }
                    if ui.button("×").clicked() {
                        remove_idx = Some(idx);
                    }
                });
            }
            if let Some(idx) = remove_idx {
                rs.vocab.remove(idx);
                rs.vocab_dirty = true;
            }
        }
        SidebarSection::Sentences => {
            let mut remove_idx: Option<usize> = None;
            for (idx, s) in rs.sentences.iter().enumerate() {
                ui.horizontal(|ui| {
                    let preview: String = s.text.chars().take(40).collect();
                    if ui.selectable_label(false, format!("{} (p.{})", preview, s.page + 1)).clicked() {
                        *page = s.page;
                    }
                    if ui.button("×").clicked() {
                        remove_idx = Some(idx);
                    }
                });
            }
            if let Some(idx) = remove_idx {
                rs.sentences.remove(idx);
                rs.sentences_dirty = true;
            }
        }
    }

    // Manual add vocab dialog (for EPUB/TXT without word-position selection)
    if rs.show_add_vocab_dialog {
        let mut keep = true;
        egui::Window::new(crate::app::i18n::tr(lang, "Add Vocabulary"))
            .open(&mut keep)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label(crate::app::i18n::tr(lang, "Word / Phrase:"));
                ui.add(egui::TextEdit::singleline(&mut rs.add_vocab_text)
                    .desired_width(200.0));
                ui.add_space(8.0);
                if ui.button(crate::app::i18n::tr(lang, "Save")).clicked() {
                    let text = rs.add_vocab_text.trim().to_string();
                    if !text.is_empty() {
                        rs.vocab.push(Vocabulary {
                            id: uuid::Uuid::new_v4().to_string(),
                            word: text,
                            context_sentence: None,
                            definition: None,
                            page: *page,
                        });
                        rs.vocab_dirty = true;
                    }
                    rs.show_add_vocab_dialog = false;
                    rs.add_vocab_text.clear();
                }
            });
        if !keep {
            rs.show_add_vocab_dialog = false;
            rs.add_vocab_text.clear();
        }
    }
}
