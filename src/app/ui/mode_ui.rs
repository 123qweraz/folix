use crate::app::engines::{Document, TocEntry};
use crate::app::core::mode_system::{ReadingState, ViewMode, AutoState, AnnotateState, AutoPlayMode, AnnotationTool};
use std::sync::Arc;
use parking_lot::Mutex;

pub fn render_reading(ui: &mut egui::Ui, document: &Option<Arc<Mutex<Box<dyn Document>>>>, rs: &mut ReadingState) {
    let supports_image = document.as_ref().map(|d| d.lock().supports_image()).unwrap_or(false);
    if !supports_image {
        rs.view_mode = ViewMode::Text;
    }

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
        if supports_image {
            ui.separator();
            if ui.selectable_label(rs.view_mode == ViewMode::Text, "Text").clicked() {
                rs.view_mode = ViewMode::Text;
            }
            if ui.selectable_label(rs.view_mode == ViewMode::Image, "Image").clicked() {
                rs.view_mode = ViewMode::Image;
            }
        }
        ui.separator();
        if ui.toggle_value(&mut rs.show_toc, "📖 ToC").clicked() {
        }
    });

    ui.separator();

    if let Some(ref doc) = document {
        if rs.show_toc {
            let toc = doc.lock().toc_entries();
            render_toc_panel(ui, &toc, rs);
        }
        match rs.view_mode {
            ViewMode::Text => render_native_text(ui, doc, rs),
            ViewMode::Image => render_reading_image(ui, doc, rs),
        }
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(200.0);
            ui.heading("No document open");
            ui.label("Use File → Open or Ctrl+O to open a PDF, EPUB, or TXT file.");
        });
    }
}

fn render_toc_panel(ui: &mut egui::Ui, toc: &[TocEntry], rs: &mut ReadingState) {
    egui::CollapsingHeader::new("Table of Contents")
        .default_open(true)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for entry in toc {
                        let selected = entry.page_index == rs.page;
                        let resp = ui.selectable_label(selected, &entry.label);
                        if resp.clicked() {
                            rs.page = entry.page_index;
                        }
                    }
                });
        });
    ui.separator();
}

fn render_native_text(ui: &mut egui::Ui, doc: &Arc<Mutex<Box<dyn Document>>>, rs: &mut ReadingState) {
    let text = {
        let d = doc.lock();
        d.page_text(rs.page)
    };

    egui::ScrollArea::both()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(20, 10))
                .show(ui, |ui| {
                    ui.add(egui::Label::new(&text).wrap());
                });
        });
}

fn render_reading_image(ui: &mut egui::Ui, doc: &Arc<Mutex<Box<dyn Document>>>, rs: &mut ReadingState) {
    let rendered = {
        let d = doc.lock();
        d.render_page(rs.page, rs.scale)
    };

    match rendered {
        Some(page) => {
            egui::ScrollArea::both()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [page.width as usize, page.height as usize],
                        &page.rgba,
                    );
                    let texture = ui.ctx().load_texture(
                        "pdf_page",
                        color_image,
                        egui::TextureOptions::default(),
                    );
                    ui.image((texture.id(), egui::Vec2::new(
                        page.width as f32,
                        page.height as f32,
                    )));
                });
        }
        None => {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.label("Image rendering not available for this document type.");
                ui.label("Switch to Text mode to view content.");
            });
        }
    }
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
        if aut.playing {
            let dt = ui.input(|i| i.unstable_dt);
            aut.progress += dt * aut.speed * 0.5;
            let page_count = doc.lock().page_count();
            aut.progress = (aut.progress as usize % page_count.max(1)) as f32;
            ctx.request_repaint();
        }

        let current_page = aut.progress as usize;
        let text = {
            let d = doc.lock();
            d.page_text(current_page)
        };

        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(20, 10))
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(&text).wrap());
                    });
            });
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(200.0);
            ui.heading("No document open");
        });
    }
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
        let page = 0;
        let text = {
            let d = doc.lock();
            d.page_text(page)
        };

        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(20, 10))
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(&text).wrap());
                    });
            });
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(200.0);
            ui.heading("No document open");
        });
    }
}
