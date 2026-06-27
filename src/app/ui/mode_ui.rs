use crate::app::engines::{Document, TocEntry};
use crate::app::core::mode_system::{ReadingState, ViewMode, AutoState, AnnotateState, AutoPlayMode, AnnotationTool};
use std::sync::Arc;
use parking_lot::Mutex;

pub fn render_reading(ui: &mut egui::Ui, document: &Option<Arc<Mutex<Box<dyn Document>>>>, rs: &mut ReadingState) {
    let supports_image = document.as_ref().map(|d| d.lock().supports_image()).unwrap_or(false);
    if !supports_image {
        rs.view_mode = ViewMode::Text;
    }

    if supports_image {
        ui.horizontal(|ui| {
            if ui.button("◀ Prev").clicked() && rs.page > 0 {
                rs.page -= 1;
            }
            ui.label(format!("Page {}/{}", rs.page + 1, {
                document.as_ref().map(|d| d.lock().page_count()).unwrap_or(0)
            }));
            if ui.button("Next ▶").clicked() {
                if let Some(ref doc) = document {
                    if rs.page + 1 < doc.lock().page_count() {
                        rs.page += 1;
                    }
                }
            }
            ui.separator();
            ui.label("Zoom:");
            ui.add(egui::Slider::new(&mut rs.scale, 0.5..=3.0).text("x"));
            ui.separator();
            if ui.selectable_label(rs.view_mode == ViewMode::Text, "Text").clicked() {
                rs.view_mode = ViewMode::Text;
            }
            if ui.selectable_label(rs.view_mode == ViewMode::Image, "Image").clicked() {
                rs.view_mode = ViewMode::Image;
            }
            ui.separator();
            if ui.toggle_value(&mut rs.show_toc, "📖 ToC").clicked() {
            }
        });
    } else {
        ui.horizontal(|ui| {
            ui.label("📖 Continuous");
            ui.separator();
            if ui.toggle_value(&mut rs.show_toc, "📖 ToC").clicked() {
            }
        });
    }

    ui.separator();

    if let Some(ref doc) = document {
        if rs.show_toc {
            let toc = doc.lock().toc_entries();
            render_toc_panel(ui, &toc, supports_image, Some(rs));
        }
        match rs.view_mode {
            ViewMode::Image if supports_image => render_reading_image(ui, doc, rs),
            _ => render_text_continuous(ui, doc),
        }
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(200.0);
            ui.heading("No document open");
            ui.label("Use File → Open or Ctrl+O to open a PDF, EPUB, or TXT file.");
        });
    }
}

fn render_toc_panel(ui: &mut egui::Ui, toc: &[TocEntry], is_pdf: bool, mut rs: Option<&mut ReadingState>) {
    egui::CollapsingHeader::new("Table of Contents")
        .default_open(true)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for entry in toc {
                        let label = if is_pdf {
                            format!("{} (p.{})", entry.label, entry.page_index + 1)
                        } else {
                            entry.label.clone()
                        };
                        if is_pdf {
                            let is_selected = rs.as_ref().map_or(false, |r| r.page == entry.page_index);
                            if ui.selectable_label(is_selected, &label).clicked() {
                                if let Some(ref mut r) = rs {
                                    r.page = entry.page_index;
                                }
                            }
                        } else {
                            ui.label(&label);
                        }
                    }
                });
        });
    ui.separator();
}

fn render_text_continuous(ui: &mut egui::Ui, doc: &Arc<Mutex<Box<dyn Document>>>) {
    let text = {
        let d = doc.lock();
        d.page_text(0)
    };

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(20, 10))
                .show(ui, |ui| {
                    ui.add(egui::Label::new(&text).wrap());
                });
        });
}

fn render_page_image(ui: &mut egui::Ui, doc: &Arc<Mutex<Box<dyn Document>>>, page: usize, scale: f32) {
    let page_data = {
        let d = doc.lock();
        d.render_page(page, scale)
    };

    match page_data {
        Some(p) => {
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [p.width as usize, p.height as usize],
                &p.rgba,
            );
            let texture = ui.ctx().load_texture(
                "doc_page",
                color_image,
                egui::TextureOptions::default(),
            );
            ui.image((texture.id(), egui::Vec2::new(p.width as f32, p.height as f32)));
        }
        None => {
            ui.label("Image not available.");
        }
    }
}

fn render_continuous_images(ui: &mut egui::Ui, doc: &Arc<Mutex<Box<dyn Document>>>, page: &mut usize, scale: f32, total: usize) {
    let window = 3;
    let p = *page;
    let start = p.saturating_sub(window);
    let end = std::cmp::min(total, p + window + 1);

    egui::ScrollArea::both()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.vertical(|ui| {
                for i in start..end {
                    render_page_image(ui, doc, i, scale);
                    if i + 1 < end {
                        ui.add_space(12.0);
                    }
                }
            });
        });
}

fn render_reading_image(ui: &mut egui::Ui, doc: &Arc<Mutex<Box<dyn Document>>>, rs: &mut ReadingState) {
    let total = {
        let d = doc.lock();
        d.page_count()
    };
    render_continuous_images(ui, doc, &mut rs.page, rs.scale, total);
}

pub fn render_auto(ui: &mut egui::Ui, document: &Option<Arc<Mutex<Box<dyn Document>>>>, aut: &mut AutoState, ctx: egui::Context) {
    ui.horizontal(|ui| {
        let play_label = if aut.playing { "⏸ Pause" } else { "▶ Play" };
        if ui.button(play_label).clicked() {
            aut.playing = !aut.playing;
        }
        ui.separator();
        ui.label("Speed:");
        ui.add(egui::Slider::new(&mut aut.speed, 0.5..=5.0).text("x"));
        ui.separator();
        ui.label("Mode:");
        egui::ComboBox::from_id_salt("auto_mode")
            .selected_text(format!("{:?}", aut.auto_mode))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut aut.auto_mode, AutoPlayMode::PageFlow, "Page Flow");
                ui.selectable_value(&mut aut.auto_mode, AutoPlayMode::GlyphReveal, "Glyph Reveal");
                ui.selectable_value(&mut aut.auto_mode, AutoPlayMode::SentenceStream, "Sentence Stream");
            });
    });

    ui.separator();

    if let Some(ref doc) = document {
        let supports_image = doc.lock().supports_image();

        if supports_image {
            // PDF: page-based auto play
            if aut.playing {
                let dt = ui.input(|i| i.unstable_dt);
                let page_count = doc.lock().page_count();
                aut.progress += dt * aut.speed * 0.5;
                if aut.progress >= page_count as f32 && page_count > 0 {
                    aut.progress -= page_count as f32;
                }
                ctx.request_repaint();
            }

            let current_page = aut.progress as usize;
            ui.horizontal(|ui| {
                ui.label(format!("Page {}/{}", current_page + 1, {
                    doc.lock().page_count()
                }));
            });
            let total = doc.lock().page_count();
            let mut auto_page = current_page;
            render_continuous_images(ui, doc, &mut auto_page, 1.0, total);
        } else {
            // TXT/EPUB: continuous scroll auto play
            if aut.playing {
                let dt = ui.input(|i| i.unstable_dt);
                aut.progress += dt * aut.speed * 0.05;
                if aut.progress >= 1.0 {
                    aut.progress -= 1.0;
                }
                ctx.request_repaint();
            }

            ui.horizontal(|ui| {
                ui.label(format!("Progress: {:.1}%", aut.progress * 100.0));
            });
            render_auto_text(ui, doc, aut);
        }
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(200.0);
            ui.heading("No document open");
        });
    }
}

fn render_auto_text(ui: &mut egui::Ui, doc: &Arc<Mutex<Box<dyn Document>>>, aut: &mut AutoState) {
    let full_text = {
        let d = doc.lock();
        d.page_text(0)
    };
    let total_chars = full_text.chars().count();
    if total_chars == 0 {
        return;
    }

    let avail_w = ui.available_width().max(100.0);
    let avail_h = ui.available_height().max(100.0);

    // Rough estimate: CJK avg ~12px wide, line height ~22px
    let chars_per_line = (avail_w / 12.0) as usize;
    let lines_per_view = (avail_h / 22.0) as usize;
    let viewport_chars = (chars_per_line * lines_per_view).max(200);

    let start_char = (aut.progress * total_chars as f32) as usize;
    let end_char = std::cmp::min(total_chars, start_char + viewport_chars);

    let slice: String = full_text.chars().skip(start_char).take(end_char - start_char).collect();

    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(20, 10))
        .show(ui, |ui| {
            ui.add(egui::Label::new(&slice).wrap());
        });
}

pub fn render_annotate(ui: &mut egui::Ui, document: &Option<Arc<Mutex<Box<dyn Document>>>>, an: &mut AnnotateState) {
    ui.horizontal(|ui| {
        ui.label("Tool:");
        let tools = [
            (AnnotationTool::Highlight, "🖊 Highlight"),
            (AnnotationTool::Pen, "✏ Pen"),
            (AnnotationTool::Note, "📝 Note"),
            (AnnotationTool::Eraser, "🧹 Eraser"),
            (AnnotationTool::Select, "👆 Select"),
        ];
        for (tool, label) in &tools {
            let is_selected = std::mem::discriminant(&an.tool) == std::mem::discriminant(tool);
            if ui.selectable_label(is_selected, *label).clicked() {
                an.tool = tool.clone();
            }
        }
        ui.separator();
        if ui.button("Undo").clicked() {
            an.annotations.pop();
        }
        if ui.button("Clear All").clicked() {
            an.annotations.clear();
        }
    });

    ui.separator();

    if let Some(ref doc) = document {
        let supports_image = doc.lock().supports_image();

        if supports_image {
            ui.horizontal(|ui| {
                if ui.button("◀ Prev").clicked() && an.page > 0 {
                    an.page -= 1;
                }
                ui.label(format!("Page {}/{}", an.page + 1, {
                    doc.lock().page_count()
                }));
                if ui.button("Next ▶").clicked() {
                    if an.page + 1 < doc.lock().page_count() {
                        an.page += 1;
                    }
                }
            });
            ui.separator();
            let total = doc.lock().page_count();
            let mut annotate_page = an.page;
            render_continuous_images(ui, doc, &mut annotate_page, 1.0, total);
        } else {
            render_text_continuous(ui, doc);
        }
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(200.0);
            ui.heading("No document open");
        });
    }
}
