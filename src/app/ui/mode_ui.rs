use crate::app::engines::{DocumentHandle, ContentBlock, TextWordPosition};
use crate::app::core::mode_system::{ReadingState, ReadingLayout, FitMode, ViewRotation, Bookmark, AutoState, AnnotateState, SelectionState, Annotation, AnnotationTool, Vocabulary, SidebarSection, MoYuState, LayoutRow};
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
    doc_path: Option<&str>,
) {
    let _frame_timer = std::time::Instant::now();

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
                                *page = page.saturating_add(advance).min(max_page);
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
            let next_page = page.saturating_add(1);
            if next_page < total {
                if fixed.get_texture_handle(next_page, *scale).is_none() {
                    if let Some(p) = fixed.render_page(next_page, *scale) {
                        let ci = egui::ColorImage::from_rgba_unmultiplied(
                            [p.width as usize, p.height as usize],
                            &p.rgba,
                        );
                        let tex = ui.ctx().load_texture(
                            format!("doc_page_{}", next_page),
                            ci,
                            egui::TextureOptions::default(),
                        );
                        fixed.set_texture_handle(next_page, *scale, tex);
                    }
                }
            }
        }

        let doc_id = doc_path;
        match *reading_layout {
            ReadingLayout::Paged => {
                render_paged_with_id(ui, document, *page, *scale, *view_rotation, &mut reading.selection, annotate, dark_mode, highlights, doc_id);
            }
            ReadingLayout::Scroll => {
                let total = document.lock().as_fixed().map(|f| f.page_count()).unwrap_or(0);
                if reading.scroll_velocity != 0.0 {
                    let dt = ui.input(|i| i.unstable_dt);
                    reading.scroll_offset_y =
                        (reading.scroll_offset_y + reading.scroll_velocity * dt).max(0.0);
                }
                render_scroll_with_id(ui, document, page, *scale, *view_rotation, total, &mut reading.scroll_offset_y, &mut reading.selection, annotate, dark_mode, highlights, doc_id);
                reading.scroll_velocity = 0.0;
            }
        }
    } else {
        drop(doc);

        if reading.chapter_cache.is_empty() {
            let doc_handle = document.lock();
            if let Some(reflow) = doc_handle.as_reflow() {
                let n = reflow.chapter_count();
                for ci in 0..n {
                    reading.chapter_cache.push(Some(reflow.load_chapter(ci, false)));
                }
            }
            reading.next_img_load_ci = 0;
        }

        // Phase 2: load images (1 chapter/frame) after all text is loaded
        {
            let doc_guard = document.lock();
            if let Some(reflow) = doc_guard.as_reflow() {
                while reading.next_img_load_ci < reading.chapter_cache.len() {
                    let ci = reading.next_img_load_ci;
                    reading.next_img_load_ci += 1;
                    if let Some(Some(ch)) = reading.chapter_cache.get(ci) {
                        let has_empty_images = ch.blocks.iter().any(|b| {
                            if let ContentBlock::Image(img) = b { img.raw_bytes.is_empty() } else { false }
                        });
                        if has_empty_images {
                            let t0 = std::time::Instant::now();
                            let ch = reflow.load_chapter(ci, true);
                            let img_cnt = ch.blocks.iter().filter(|b| matches!(b, ContentBlock::Image(_))).count();
                            eprintln!("[perf] loaded ch{} images: {:?} imgs={}", ci, t0.elapsed(), img_cnt);
                            reading.chapter_cache[ci] = Some(ch);
                            reading.layout_cache_rows.clear();
                            reading.layout_cache_starts.clear();
                            break; // 1 chapter per frame
                        }
                    }
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
        }
        let init_offset = reading.pending_scroll_y.take().unwrap_or(reading.scroll_offset_y);
        reading.scroll_offset_y = init_offset;
        sa = sa.vertical_scroll_offset(init_offset);

        let font_size = 16.0 * *scale;

        let output;
        // ---- Unified virtual scrolling (layout cache + painter + interaction) ----
        let full_w = ui.available_width().max(1.0);
        let mw = reading.max_text_width;
        let avail_w = if mw > 0.0 { full_w.min(mw) } else { full_w };
        let x_off = ((full_w - avail_w) * 0.5).max(0.0);
        let gutter_w = if reading.show_line_numbers { 65.0 } else { 0.0 };
        let text_avail_w = (avail_w - gutter_w).max(1.0);
        let font_id = egui::FontId::proportional(font_size);
        let line_h = font_size * 1.4;

        // ---- Layout cache management with partial rebuild + resize throttle ----
        let rows: &[LayoutRow];
        let row_starts: &[f32];

        // Helper: compute cpl-based height estimate for non-visible text rows
        let cpl_heuristic = |text: &str, ta_w: f32, fs: f32, lh: f32| -> f32 {
            let cpl = (ta_w / (fs * 0.55)).floor().max(1.0) as usize;
            let nc = text.chars().count().max(1);
            let vlines = (nc as f32 / cpl as f32).ceil().max(1.0);
            vlines * lh
        };
        let heading_font_size = |level: u8, body: f32| -> f32 {
            match level {
                1 => body * 1.75,
                2 => body * 1.5,
                3 => body * 1.25,
                4 => body * 1.1,
                5 => body * 1.0,
                6 => body * 0.875,
                _ => body,
            }
        };
        let font_family_for_row = |bold: bool, italic: bool| -> egui::FontFamily {
            if bold && italic {
                egui::FontFamily::Name("bold_italic".into())
            } else if bold {
                egui::FontFamily::Name("bold".into())
            } else if italic {
                egui::FontFamily::Name("italic".into())
            } else {
                egui::FontFamily::Proportional
            }
        };

        // Lazy update: recompute galley for rows that scrolled into view
        // (runs BEFORE rows/row_starts borrow; fixes rows whose galley is None
        //  because they were outside the visible range during the last partial build)
        if reading.layout_cache_gen > 0 {
            let gen = reading.layout_cache_gen;
            let cull_min = (reading.scroll_offset_y - ui.available_height() * 0.5).max(0.0);
            let cull_max = reading.scroll_offset_y + ui.available_height() * 1.5;
            let first = reading.layout_cache_starts.partition_point(|&y| y < cull_min);
            let last = reading.layout_cache_starts.partition_point(|&y| y < cull_max)
                .min(reading.layout_cache_rows.len());
            let mut any_change = false;
            for i in first..last {
                let row = &mut reading.layout_cache_rows[i];
                if (row.it == 1 || row.it == 4) && !row.text.is_empty() && row.galley.is_none() && row.layout_gen == gen {
                    let actual_fs = heading_font_size(row.heading_level, font_size);
                    let actual_fid = egui::FontId::new(actual_fs, font_family_for_row(row.bold, row.italic));
                    let g = ui.fonts(|f| f.layout_delayed_color(
                        row.text.clone(),
                        actual_fid,
                        text_avail_w));
                    row.height = g.rect.height().max(1.0);
                    row.galley = Some(g.into());
                    any_change = true;
                }
            }
            if any_change {
                let mut acc_y = if first > 0 { reading.layout_cache_starts[first] } else { 0.0 };
                for i in first..reading.layout_cache_rows.len() {
                    reading.layout_cache_starts[i] = acc_y;
                    acc_y += reading.layout_cache_rows[i].height;
                }
                reading.total_height = acc_y;
            }
        }

        // ---- Cache selection ----
        let cache_hit = reading.layout_cache_font_size == font_size
            && reading.layout_cache_avail_w == avail_w
            && reading.layout_cache_show_ln == reading.show_line_numbers
            && !reading.layout_cache_rows.is_empty();

        if cache_hit {
            rows = &reading.layout_cache_rows;
            row_starts = &reading.layout_cache_starts;
        } else if reading.layout_cache_font_size == font_size
            && reading.layout_cache_show_ln == reading.show_line_numbers
            && !reading.layout_cache_rows.is_empty()
            && reading.layout_cache_pending_avail_w != avail_w
        {
            // Resize throttle: skip rebuild during active drag
            reading.layout_cache_pending_avail_w = avail_w;
            rows = &reading.layout_cache_rows;
            row_starts = &reading.layout_cache_starts;
        } else {
            // Rebuild: first load, or font_size / show_ln / avail_w changed
            reading.layout_cache_pending_avail_w = 0.0;
            reading.layout_cache_gen = reading.layout_cache_gen.wrapping_add(1);
            reading.layout_cache_font_size = font_size;
            reading.layout_cache_avail_w = avail_w;
            reading.layout_cache_show_ln = reading.show_line_numbers;
            let gen = reading.layout_cache_gen;

            // First load: create all rows from scratch with cpl-estimated heights
            if reading.layout_cache_rows.is_empty() {
                let mut new_rows: Vec<LayoutRow> = Vec::new();
                let mut global_line: usize = 0;
                // Compute average loaded chapter height for placeholder sizing
                let mut ph_sum = 0.0f32;
                let mut ph_cnt = 0usize;
                for (_, ch_opt) in reading.chapter_cache.iter().enumerate() {
                    if let Some(ch) = ch_opt.as_ref() {
                        for b in &ch.blocks {
                            ph_sum += match b {
                                ContentBlock::Text { text: t, .. } | ContentBlock::Link { text: t, .. } => cpl_heuristic(t, text_avail_w, font_size, line_h),
                                ContentBlock::Image(img) => {
                                    let max_w = (avail_w.min(600.0)).min(img.width.max(1) as f32);
                                    let aspect = img.width as f32 / img.height.max(1) as f32;
                                    max_w / aspect + 8.0
                                }
                            };
                        }
                        ph_cnt += 1;
                    }
                }
                let ph = if ph_cnt > 0 { (ph_sum / ph_cnt as f32).max(200.0) } else { 600.0 };

                for (ci, chapter_opt) in reading.chapter_cache.iter().enumerate() {
                    if ci > 0 {
                        new_rows.push(LayoutRow { line_no: 0, ci, bi: 0, it: 0, text: String::new(), height: 12.0, char_offset: 0, galley: None, layout_gen: gen, heading_level: 0, bold: false, italic: false, list_item: false, target_ci: None });
                    }
                    let chapter = match chapter_opt.as_ref() {
                        Some(ch) => ch,
                        None => {
                            new_rows.push(LayoutRow { line_no: global_line + 1, ci, bi: 0, it: 3, text: String::new(), height: ph, char_offset: 0, galley: None, layout_gen: gen, heading_level: 0, bold: false, italic: false, list_item: false, target_ci: None });
                            global_line += 1;
                            continue;
                        }
                    };
                    for (bi, block) in chapter.blocks.iter().enumerate() {
                        match block {
                            ContentBlock::Text { text, heading_level, bold, italic, list_item } => {
                                let lines: Vec<&str> = text.split('\n').collect();
                                let mut char_offset = 0;
                                for (li, src_line) in lines.iter().enumerate() {
                                    let lno = global_line + 1;
                                    let actual_fs = heading_font_size(*heading_level, font_size);
                                    let actual_lh = actual_fs * 1.4;
                                    let h = if src_line.is_empty() { actual_lh } else { cpl_heuristic(src_line, text_avail_w, actual_fs, actual_lh) };
                                    new_rows.push(LayoutRow {
                                        line_no: lno, ci, bi, it: 1,
                                        text: src_line.to_string(),
                                        height: h,
                                        char_offset,
                                        galley: None,
                                        layout_gen: gen,
                                        heading_level: *heading_level,
                                        bold: *bold,
                                        italic: *italic,
                                        list_item: *list_item,
                                        target_ci: None,
                                    });
                                    char_offset += src_line.chars().count();
                                    if li < lines.len() - 1 { char_offset += 1; }
                                    global_line += 1;
                                }
                            }
                            ContentBlock::Image(img) => {
                                let max_w = avail_w.min(600.0);
                                let aspect = img.width as f32 / img.height.max(1) as f32;
                                new_rows.push(LayoutRow {
                                    line_no: global_line + 1, ci, bi, it: 2,
                                    text: String::new(),
                                    height: max_w / aspect + 8.0,
                                    char_offset: 0,
                                    galley: None,
                                    layout_gen: gen,
                                    heading_level: 0,
                                    bold: false,
                                    italic: false,
                                    list_item: false,
                                    target_ci: None,
                                });
                                global_line += 1;
                            }
                            ContentBlock::Link { text, target_ci } => {
                                for src_line in text.split('\n') {
                                    let lno = global_line + 1;
                                    let h = if src_line.is_empty() { line_h } else { cpl_heuristic(src_line, text_avail_w, font_size, line_h) };
                                    new_rows.push(LayoutRow {
                                        line_no: lno, ci, bi, it: 4,
                                        text: src_line.to_string(),
                                        height: h,
                                        char_offset: 0,
                                        galley: None,
                                        layout_gen: gen,
                                        heading_level: 0,
                                        bold: false,
                                        italic: false,
                                        list_item: false,
                                        target_ci: *target_ci,
                                    });
                                    global_line += 1;
                                }
                            }
                        }
                    }
                }
                let mut new_starts = Vec::with_capacity(new_rows.len());
                let mut acc_y = 0.0;
                for r in &new_rows {
                    new_starts.push(acc_y);
                    acc_y += r.height;
                }
                reading.layout_cache_rows = new_rows;
                reading.layout_cache_starts = new_starts;
            }

            // Save top row index
            let top_idx = if reading.layout_cache_rows.len() > 0 {
                reading.layout_cache_starts
                    .partition_point(|&y| y <= reading.scroll_offset_y)
                    .saturating_sub(1)
                    .min(reading.layout_cache_rows.len().saturating_sub(1))
            } else {
                0
            };

            // Estimate visible range
            let approx_vh = ui.available_height();
            let margin = approx_vh * 0.5;
            let cull_min = (reading.scroll_offset_y - margin).max(0.0);
            let cull_max = reading.scroll_offset_y + approx_vh + margin;
            let vis_first = reading.layout_cache_starts.partition_point(|&y| y < cull_min);
            let vis_last = reading.layout_cache_starts.partition_point(|&y| y < cull_max)
                .min(reading.layout_cache_rows.len());

            // Update heights for ALL rows (cpl fast path for non-visible, exact for visible)
            for i in 0..reading.layout_cache_rows.len() {
                let row = &mut reading.layout_cache_rows[i];
                row.layout_gen = gen;
                match row.it {
                    1 | 4 => {
                        if row.text.is_empty() {
                            row.height = line_h;
                            row.galley = None;
                        } else if i >= vis_first && i < vis_last {
                            let actual_fs = heading_font_size(row.heading_level, font_size);
                            let actual_fid = egui::FontId::new(actual_fs, font_family_for_row(row.bold, row.italic));
                            let g = ui.fonts(|f| f.layout_delayed_color(
                                row.text.clone(),
                                actual_fid,
                                text_avail_w));
                            row.height = g.rect.height().max(1.0);
                            row.galley = Some(g.into());
                        } else {
                            let actual_fs = heading_font_size(row.heading_level, font_size);
                            let actual_lh = actual_fs * 1.4;
                            row.height = cpl_heuristic(&row.text, text_avail_w, actual_fs, actual_lh);
                            row.galley = None;
                        }
                    }
                    2 => {
                        if let Some(Some(ch)) = reading.chapter_cache.get(row.ci) {
                            if let Some(ContentBlock::Image(img)) = ch.blocks.get(row.bi) {
                                let max_w = (avail_w.min(600.0)).min(img.width.max(1) as f32);
                                let aspect = img.width as f32 / img.height.max(1) as f32;
                                row.height = max_w / aspect + 8.0;
                            }
                        }
                        row.galley = None;
                    }
                    3 => { row.galley = None; }
                    _ => { row.height = 12.0; row.galley = None; }
                }
            }

            // Recompute row_starts from scratch, fix scroll_offset_y
            let mut acc_y = 0.0;
            for i in 0..reading.layout_cache_rows.len() {
                reading.layout_cache_starts[i] = acc_y;
                acc_y += reading.layout_cache_rows[i].height;
            }
            if top_idx < reading.layout_cache_rows.len() {
                reading.scroll_offset_y = reading.layout_cache_starts[top_idx];
            }
            reading.total_height = acc_y;

            rows = &reading.layout_cache_rows;
            row_starts = &reading.layout_cache_starts;
        }

        reading.total_height = row_starts.last().map_or(0.0, |&last| last)
            + rows.last().map_or(0.0, |r| r.height);

        let chapter_cache_ref = &reading.chapter_cache;
        let text_color = ui.style().visuals.text_color();
        let img_max_w = (avail_w - gutter_w).min(600.0);

        output = sa.show(ui, |ui| {
            let total_h = reading.total_height;
            let (response, painter) = ui.allocate_painter(
                egui::vec2(ui.available_width(), total_h.max(0.0)),
                egui::Sense::empty(),
            );

            // Virtual scrolling: only paint rows in the visible region + margin
            let clip = ui.clip_rect();
            let content_origin = response.rect.top();
            let visible_top = (clip.top() - content_origin).max(0.0);
            let visible_bottom = (clip.bottom() - content_origin).min(total_h);
            let margin = (visible_bottom - visible_top) * 0.5;
            let cull_min = (visible_top - margin).max(0.0);
            let cull_max = (visible_bottom + margin).min(total_h);

            let mut first = row_starts.partition_point(|&y| y < cull_min);
            if first > 0 && row_starts[first - 1] + rows[first - 1].height > cull_min {
                first -= 1;
            }
            let last = row_starts.partition_point(|&y| y < cull_max);

            let base_x = response.rect.left();
            let base_y = content_origin;
            let alloc_w = response.rect.width();
            let content_left = base_x + x_off;
            let ann_ref = annotate.as_ref();

            // Paint pass
            for i in first..last.min(rows.len()) {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(base_x, base_y + row_starts[i]),
                    egui::vec2(alloc_w, rows[i].height),
                );

                match rows[i].it {
                    0 => {
                        let sep_y = rect.center().y;
                        painter.line_segment(
                            [egui::pos2(rect.left(), sep_y), egui::pos2(rect.right(), sep_y)],
                            ui.visuals().widgets.noninteractive.bg_stroke,
                        );
                    }
                    1 => {
                        if reading.show_line_numbers {
                            painter.text(
                                egui::pos2(content_left + 4.0, rect.top()),
                                egui::Align2::LEFT_TOP,
                                format!("{:>6}│ ", rows[i].line_no),
                                font_id.clone(),
                                egui::Color32::GRAY,
                            );
                        }

                        if let Some(galley) = &rows[i].galley {
                            let text_x = content_left + gutter_w;
                            painter.add(egui::Shape::galley(
                                egui::pos2(text_x, rect.top()),
                                galley.clone(),  // cheap Arc clone
                                text_color,
                            ));
                        }
                    }
                    3 => {
                        painter.rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(content_left + gutter_w, rect.top() + 4.0),
                                egui::vec2(img_max_w, (rect.height() - 8.0).max(0.0)),
                            ),
                            4.0,
                            egui::Color32::from_gray(238),
                        );
                        painter.text(
                            egui::pos2(content_left + gutter_w + 8.0, rect.top() + 8.0),
                            egui::Align2::LEFT_TOP,
                            "加载中...",
                            egui::FontId::proportional(12.0),
                            egui::Color32::GRAY,
                        );
                    }
                    4 => {
                        if reading.show_line_numbers {
                            painter.text(
                                egui::pos2(content_left + 4.0, rect.top()),
                                egui::Align2::LEFT_TOP,
                                format!("{:>6}│ ", rows[i].line_no),
                                font_id.clone(),
                                egui::Color32::GRAY,
                            );
                        }
                        let link_color = egui::Color32::from_rgb(30, 100, 220);
                        if let Some(galley) = &rows[i].galley {
                            let text_x = content_left + gutter_w;
                            let text_top = rect.top();
                            painter.add(egui::Shape::galley(
                                egui::pos2(text_x, text_top),
                                galley.clone(),
                                link_color,
                            ));
                            // underline
                            let line_y = rect.bottom() - 2.0;
                            painter.line_segment(
                                [egui::pos2(text_x, line_y), egui::pos2(text_x + galley.rect.width(), line_y)],
                                (1.0, link_color),
                            );
                        }
                    }
                    _ => {
                        // If image raw_bytes not loaded yet, draw a placeholder
                        let ch = &chapter_cache_ref[rows[i].ci];
                        let img_data = ch.as_ref().and_then(|ch| {
                            if rows[i].bi < ch.blocks.len() {
                                if let ContentBlock::Image(img) = &ch.blocks[rows[i].bi] { Some(img) } else { None }
                            } else {
                                None
                            }
                        });
                        let is_empty = img_data.map_or(true, |img| img.raw_bytes.is_empty());
                        if is_empty {
                            // On-demand: prioritize this chapter for Phase 2 image loading
                            if reading.next_img_load_ci > rows[i].ci {
                                reading.next_img_load_ci = rows[i].ci;
                            }
                            if let Some(ch) = ch.as_ref() {
                                if rows[i].bi < ch.blocks.len() {
                                    let kind = match &ch.blocks[rows[i].bi] {
                                        ContentBlock::Text { .. } => "Text",
                                        ContentBlock::Image(_) => "Image",
                                        ContentBlock::Link { .. } => "Link",
                                    };
                                    eprintln!("[dbg] ci={} bi={} it={} kind={}", rows[i].ci, rows[i].bi, rows[i].it, kind);
                                }
                            }
                            let aspect = img_data.map_or(1.0, |img| {
                                if img.height > 0 { img.width as f32 / img.height as f32 } else { 1.0 }
                            });
                            let p_h = img_max_w / aspect.max(0.01);
                            if reading.show_line_numbers {
                                painter.text(
                                    egui::pos2(content_left + 4.0, rect.top()),
                                    egui::Align2::LEFT_TOP,
                                    format!("{:>6}│ ", rows[i].line_no),
                                    font_id.clone(),
                                    egui::Color32::GRAY,
                                );
                            }
                            painter.rect_filled(
                                egui::Rect::from_min_size(
                                    egui::pos2(content_left + gutter_w, rect.top() + 4.0),
                                    egui::vec2(img_max_w, p_h),
                                ),
                                4.0,
                                egui::Color32::from_gray(230),
                            );
                            continue;
                        }

                        let doc_prefix = doc_path.unwrap_or("_");
                        let key = format!("{}_epub_img_{}_{}", doc_prefix, rows[i].ci, rows[i].bi);
                        if !image_cache.contains_key(&key) {
                            let img = img_data.unwrap();
                            let decoded = match image::load_from_memory(&img.raw_bytes) {
                                Ok(d) => d.into_rgba8(),
                                Err(_) => {
                                    let placeholder = ui.ctx().load_texture(
                                        &key,
                                        egui::ColorImage::new([1, 1], egui::Color32::RED),
                                        egui::TextureOptions::default(),
                                    );
                                    image_cache.insert(key.clone(), placeholder);
                                    evict_cache(image_cache, 128);
                                    continue;
                                }
                            };
                            let (native_w, native_h) = decoded.dimensions();
                            let aspect = native_w as f32 / native_h as f32;
                            let display_w = (img_max_w.min(native_w as f32)).ceil() as u32;
                            let display_h = (display_w as f32 / aspect).ceil() as u32;
                            let resized = if display_w < native_w {
                                image::imageops::resize(
                                    &decoded,
                                    display_w.max(1),
                                    display_h.max(1),
                                    image::imageops::FilterType::Triangle,
                                )
                            } else {
                                decoded
                            };
                            let (rw, rh) = resized.dimensions();
                            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                [rw as usize, rh as usize],
                                resized.as_raw(),
                            );
                            let tex = ui.ctx().load_texture(&key, color_image, egui::TextureOptions::default());
                            image_cache.insert(key.clone(), tex);
                            evict_cache(image_cache, 128);
                        }
                        let texture = image_cache.get(&key).unwrap();

                        if reading.show_line_numbers {
                            painter.text(
                                egui::pos2(content_left + 4.0, rect.top()),
                                egui::Align2::LEFT_TOP,
                                format!("{:>6}│ ", rows[i].line_no),
                                font_id.clone(),
                                egui::Color32::GRAY,
                            );
                        }

                        let Some(ch) = chapter_cache_ref[rows[i].ci].as_ref() else { continue; };
                        let Some(ContentBlock::Image(img)) = ch.blocks.get(rows[i].bi) else { continue; };
                        let aspect = img.width as f32 / img.height.max(1) as f32;
                        let img_w = img_max_w.min(img.width.max(1) as f32);
                        let img_h = img_w / aspect;
                        painter.image(
                            texture.id(),
                            egui::Rect::from_min_size(
                                egui::pos2(content_left + gutter_w, rect.top() + 4.0),
                                egui::vec2(img_w, img_h),
                            ),
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                }
            }

            // Interaction layer (only for non-line-number mode)
            if !reading.show_line_numbers {
                let sel = &mut reading.selection;
                for i in first..last.min(rows.len()) {
                    if rows[i].it == 4 {
                        let text_rect = egui::Rect::from_min_size(
                            egui::pos2(content_left + gutter_w, base_y + row_starts[i]),
                            egui::vec2(text_avail_w, rows[i].height),
                        );
                        let resp = ui.interact(text_rect, egui::Id::new(("link", i)), egui::Sense::click());
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        if resp.clicked() {
                            if let Some(tci) = rows[i].target_ci {
                                let target_y = reading.layout_cache_starts.iter()
                                    .zip(rows.iter())
                                    .find(|(_, r)| r.ci >= tci)
                                    .map(|(&y, _)| y)
                                    .unwrap_or(reading.scroll_offset_y);
                                reading.pending_scroll_y = Some(target_y);
                            }
                        }
                        continue;
                    }
                    if rows[i].it != 1 || rows[i].text.is_empty() { continue; }
                    let text_rect = egui::Rect::from_min_size(
                        egui::pos2(content_left + gutter_w, base_y + row_starts[i]),
                        egui::vec2(text_avail_w, rows[i].height),
                    );
                    let resp = ui.interact(text_rect, egui::Id::new(("row", i)), egui::Sense::click_and_drag());

                    // Highlight rendering
                    if let Some(ann) = ann_ref {
                        for h in &ann.annotations {
                            if h.kind != AnnotationTool::Highlight { continue; }
                            if let Some((h_ch, h_blk, h_cs, h_ce)) = h.reflow_range {
                                if h_ch == rows[i].ci && h_blk == rows[i].bi {
                                    let row_start = rows[i].char_offset;
                                    let text_len = rows[i].text.chars().count();
                                    let row_end = row_start + text_len;
                                    let h_start = h_cs.max(row_start).min(row_end);
                                    let h_end = h_ce.max(row_start).min(row_end);
                                    if h_start < h_end {
                                        let local_s = h_start - row_start;
                                        let local_e = h_end - row_start;
                                        let before: String = rows[i].text.chars().take(local_s).collect();
                                        let overlap: String = rows[i].text.chars().skip(local_s).take(local_e - local_s).collect();
                                        let before_w = ui.fonts(|f| f.layout_no_wrap(before, font_id.clone(), egui::Color32::WHITE).rect.width());
                                        let overlap_w = ui.fonts(|f| f.layout_no_wrap(overlap, font_id.clone(), egui::Color32::WHITE).rect.width());
                                        let c = h.color;
                                        let hl_rect = egui::Rect::from_min_size(
                                            egui::pos2(text_rect.left() + before_w, text_rect.top()),
                                            egui::vec2(overlap_w, text_rect.height()),
                                        );
                                        ui.painter().rect_filled(hl_rect, 0.0, egui::Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]));
                                    }
                                }
                            }
                        }
                    }

                    // Selection rendering
                    if sel.selecting && sel.char_anchor.is_some() {
                        let (a_ch, a_blk, a_pos) = sel.char_anchor.unwrap();
                        let (f_ch, f_blk, f_pos) = sel.char_focus.unwrap_or((a_ch, a_blk, a_pos));
                        if (a_ch == rows[i].ci && a_blk == rows[i].bi) || (f_ch == rows[i].ci && f_blk == rows[i].bi) {
                            let row_start = rows[i].char_offset;
                            let text_len = rows[i].text.chars().count();
                            let row_end = row_start + text_len;
                            let s_start = (if a_ch == rows[i].ci && a_blk == rows[i].bi { a_pos } else { 0 }).max(row_start).min(row_end);
                            let s_end = (if f_ch == rows[i].ci && f_blk == rows[i].bi { f_pos } else { row_end }).max(row_start).min(row_end);
                            let s_low = s_start.min(s_end);
                            let s_high = s_start.max(s_end);
                            if s_low < s_high {
                                let local_s = s_low - row_start;
                                let local_e = s_high - row_start;
                                let before_text: String = rows[i].text.chars().take(local_s).collect();
                                let sel_text: String = rows[i].text.chars().skip(local_s).take(local_e - local_s).collect();
                                let before_w = ui.fonts(|f| f.layout_no_wrap(before_text, font_id.clone(), egui::Color32::WHITE).rect.width());
                                let sel_w = ui.fonts(|f| f.layout_no_wrap(sel_text, font_id.clone(), egui::Color32::WHITE).rect.width());
                                let sel_rect = egui::Rect::from_min_size(
                                    egui::pos2(text_rect.left() + before_w, text_rect.top()),
                                    egui::vec2(sel_w, text_rect.height()),
                                );
                                ui.painter().rect_filled(sel_rect, 0.0, egui::Color32::from_rgba_premultiplied(100, 150, 255, 100));
                            }
                        }
                    }

                    // Helper: get character position from mouse position
                    let char_pos_from_mouse = |mouse_x: f32, row: &LayoutRow| -> usize {
                        if let Some(galley) = &row.galley {
                            let local_x = mouse_x - (content_left + gutter_w);
                            let cursor = galley.cursor_from_pos(egui::vec2(local_x.max(0.0), 0.0));
                            row.char_offset + cursor.ccursor.index
                        } else {
                            let local_x = mouse_x - text_rect.left();
                            let ratio = (local_x / text_rect.width().max(1.0)).clamp(0.0, 1.0);
                            row.char_offset + (ratio * row.text.chars().count() as f32) as usize
                        }
                    };

                    if resp.clicked() {
                        if let Some(mouse_pos) = resp.interact_pointer_pos() {
                            let abs_char = char_pos_from_mouse(mouse_pos.x, &rows[i]);
                            sel.char_anchor = Some((rows[i].ci, rows[i].bi, abs_char));
                            sel.char_focus = Some((rows[i].ci, rows[i].bi, abs_char));
                            sel.selected_text = String::new();
                            sel.selected_word_indices.clear();
                            sel.selecting = true;
                        }
                    }

                    if resp.dragged() && sel.selecting {
                        if let Some(mouse_pos) = resp.interact_pointer_pos() {
                            let abs_char = char_pos_from_mouse(mouse_pos.x, &rows[i]);
                            sel.char_focus = Some((rows[i].ci, rows[i].bi, abs_char));
                        }
                    }

                    if resp.drag_stopped() { sel.selecting = false; }

                    resp.context_menu(|ui| {
                        if let (Some(anchor), Some(focus)) = (sel.char_anchor, sel.char_focus) {
                            let (a_ch, a_blk, a_pos) = anchor;
                            let (f_ch, f_blk, f_pos) = focus;
                            let block_text = chapter_cache_ref[rows[i].ci].as_ref()
                                .and_then(|ch| ch.blocks.get(rows[i].bi))
                                .and_then(|b| if let ContentBlock::Text { text: t, .. } = b { Some(t.as_str()) } else { None })
                                .unwrap_or("");
                            let block_len = block_text.chars().count();
                            let local_a = if a_ch == rows[i].ci && a_blk == rows[i].bi { a_pos } else { 0 };
                            let local_f = if f_ch == rows[i].ci && f_blk == rows[i].bi { f_pos } else { block_len };
                            let s_start = local_a.min(local_f).max(0).min(block_len);
                            let s_end = local_a.max(local_f).max(0).min(block_len);
                            if s_start < s_end {
                                let sel_text: String = block_text.chars().skip(s_start).take(s_end - s_start).collect();
                                if ui.button("Copy").clicked() { ui.ctx().copy_text(sel_text.clone()); ui.close_menu(); }
                                if ui.button("Add to Vocabulary").clicked() { sel.pending_vocab = Some(sel_text.clone()); ui.close_menu(); }
                                if ui.button("Save Sentence").clicked() { sel.pending_sentence = Some(sel_text); ui.close_menu(); }
                            }
                        }
                    });
                }
            }
        });

        // Update current line / total lines for toolbar display
        if !rows.is_empty() {
            let idx = row_starts.partition_point(|&y| y <= output.state.offset.y).min(rows.len());
            reading.current_line = if idx > 0 && idx <= rows.len() {
                rows[idx - 1].line_no
            } else if !rows.is_empty() {
                rows[0].line_no
            } else {
                0
            };
            reading.total_lines = rows.last().map_or(0, |r| r.line_no);
        }

        reading.scroll_offset_y = output.state.offset.y;
        reading.scroll_velocity = 0.0;

        if let Some(target) = reading.stream_jump_to.take() {
            if target > 0 && !reading.layout_cache_rows.is_empty() {
                jump_to_line(reading, target);
                // Override the ScrollArea offset by setting a pending scroll for next frame
                reading.pending_scroll_y = Some(reading.scroll_offset_y);
            } else {
                reading.stream_jump_to = Some(target);
            }
        }
    }
    eprintln!("[perf] frame: {:?}", _frame_timer.elapsed());
}

pub fn jump_to_line(reading: &mut ReadingState, target: usize) {
    if target == 0 || reading.layout_cache_rows.is_empty() {
        return;
    }
    for (i, row) in reading.layout_cache_rows.iter().enumerate() {
        if row.line_no >= target {
            reading.scroll_offset_y = reading.layout_cache_starts.get(i).copied().unwrap_or(0.0);
            return;
        }
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
    render_paged_with_id(ui, doc, page, scale, view_rotation, selection, annotate, dark_mode, highlights, None)
}

fn render_paged_with_id(
    ui: &mut egui::Ui,
    doc: &Arc<Mutex<DocumentHandle>>,
    page: usize,
    scale: f32,
    view_rotation: ViewRotation,
    selection: &mut SelectionState,
    annotate: Option<&mut AnnotateState>,
    dark_mode: bool,
    highlights: &std::collections::HashMap<usize, Vec<usize>>,
    doc_id: Option<&str>,
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
            render_image_page(ui, doc, page, scale, view_rotation, &all_words, selection, annotate, dark_mode, highlights, doc_id);
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
    annotate: Option<&mut AnnotateState>,
    dark_mode: bool,
    highlights: &std::collections::HashMap<usize, Vec<usize>>,
) {
    render_scroll_with_id(ui, doc, page, scale, view_rotation, total, out_scroll_y, selection, annotate, dark_mode, highlights, None)
}

fn render_scroll_with_id(
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
    doc_id: Option<&str>,
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
                render_image_page(ui, doc, i, scale, view_rotation, &all_words, selection, an, dark_mode, highlights, doc_id);
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
    doc_id: Option<&str>,
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
                        doc_id: doc_id.unwrap_or("").to_string(),
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

fn evict_cache(cache: &mut HashMap<String, egui::TextureHandle>, max: usize) {
    while cache.len() > max {
        let first = cache.keys().next().unwrap().clone();
        cache.remove(&first);
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
                                if document.lock().is_fixed() {
                                    rs.scroll_offset_y = 0.0;
                                } else {
                                    rs.scroll_offset_y = rs.layout_cache_rows.iter()
                                        .zip(rs.layout_cache_starts.iter())
                                        .find(|(row, _)| row.ci == target_page)
                                        .map(|(_, &y)| y)
                                        .unwrap_or(0.0);
                                }
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

    // Fill remaining vertical space so sidebar background covers the full panel height
    ui.allocate_space(egui::vec2(0.0, ui.available_height().max(0.0)));
}
