use egui;
use crate::app::core::pdf_toolbox::{PdfToolboxState, PdfOperation, SplitMode, LogEntry};
use crate::app::engines::pdf_operations;

pub fn render_pdf_toolbox(ui: &mut egui::Ui, state: &mut PdfToolboxState) {
    egui::TopBottomPanel::top("toolbox_header").show_inside(ui, |ui| {
        ui.heading("PDF Operations");
        ui.separator();
    });

    egui::SidePanel::left("toolbox_input")
        .resizable(true)
        .default_width(280.0)
        .show_inside(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("INPUT").strong().size(14.0));
                ui.separator();

                if ui.add_sized([ui.available_width(), 28.0], egui::Button::new("📂 Add Files"))
                    .clicked()
                {
                    let mut dialog = rfd::FileDialog::new();
                    match state.operation {
                        PdfOperation::ImageToPdf => {
                            dialog = dialog.add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "tiff", "tif", "webp"]);
                        }
                        _ => {
                            dialog = dialog.add_filter("PDF", &["pdf"]);
                        }
                    }
                    if let Some(files) = dialog.pick_files() {
                        for f in files {
                            let path = f.to_string_lossy().to_string();
                            if !state.input_files.contains(&path) {
                                state.input_files.push(path);
                            }
                        }
                    }
                }

                ui.add_space(4.0);

                if state.input_files.is_empty() {
                    ui.label(egui::RichText::new("No files selected").weak());
                } else {
                    let mut remove_idx = None;
                    for (i, f) in state.input_files.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let label = std::path::Path::new(f)
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or(f);
                            ui.label(egui::RichText::new(label).size(12.0));
                            if ui.small_button("×").clicked() {
                                remove_idx = Some(i);
                            }
                        });
                    }
                    if let Some(idx) = remove_idx {
                        state.input_files.remove(idx);
                        state.toc_chapters.clear();
                    }
                }

                // After file selection, load TOC for split-by-TOC
                if state.operation == PdfOperation::Split
                    && !state.input_files.is_empty()
                    && state.toc_chapters.is_empty()
                {
                    if let Ok(chapters) = pdf_operations::load_toc_chapters(&state.input_files[0]) {
                        state.toc_chapters = chapters.into_iter()
                            .map(|(t, p)| crate::app::core::pdf_toolbox::TocChapter { title: t, page: p })
                            .collect();
                    }
                }
            });
        });

    egui::SidePanel::left("toolbox_options")
        .resizable(true)
        .default_width(260.0)
        .show_inside(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("CONVERT").strong().size(14.0));
                ui.separator();

                // Operation selector
                egui::ComboBox::from_id_salt("op_selector")
                    .selected_text(state.operation_name())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.operation, PdfOperation::Merge, "Merge PDFs");
                        ui.selectable_value(&mut state.operation, PdfOperation::Split, "Split PDF");
                        ui.selectable_value(&mut state.operation, PdfOperation::ExtractImages, "Extract Images");
                        ui.selectable_value(&mut state.operation, PdfOperation::ExtractText, "Extract Text");
                        ui.selectable_value(&mut state.operation, PdfOperation::ImageToPdf, "Image(s) → PDF");
                    });

                ui.add_space(8.0);

                // Operation-specific options
                match state.operation {
                    PdfOperation::Split => {
                        ui.label("Split by:");
                        ui.radio_value(&mut state.split_mode, SplitMode::Range, "Page range");
                        ui.radio_value(&mut state.split_mode, SplitMode::EveryNPages, "Every N pages");
                        if !state.toc_chapters.is_empty() {
                            ui.radio_value(&mut state.split_mode, SplitMode::ByToc, "TOC chapters");
                        }

                        match state.split_mode {
                            SplitMode::Range => {
                                ui.horizontal(|ui| {
                                    ui.label("From:");
                                    ui.add(egui::Slider::new(&mut state.split_start, 1..=9999));
                                });
                                ui.horizontal(|ui| {
                                    ui.label("To:");
                                    ui.add(egui::Slider::new(&mut state.split_end, 1..=9999));
                                });
                                if state.split_end < state.split_start {
                                    state.split_end = state.split_start;
                                }
                            }
                            SplitMode::EveryNPages => {
                                ui.horizontal(|ui| {
                                    ui.label("Pages per chunk:");
                                    ui.add(egui::Slider::new(&mut state.split_every_n, 1..=500));
                                });
                            }
                            SplitMode::ByToc => {
                                if !state.toc_chapters.is_empty() {
                                    ui.label(format!("{} chapters found", state.toc_chapters.len()));
                                    egui::ScrollArea::vertical()
                                        .max_height(200.0)
                                        .show(ui, |ui| {
                                            for ch in &state.toc_chapters {
                                                ui.label(format!("p{}  {}", ch.page + 1, ch.title));
                                            }
                                        });
                                } else {
                                    ui.label("No TOC data. Select a PDF first.");
                                }
                            }
                        }
                    }
                    PdfOperation::ExtractImages => {
                        ui.label("Each page is exported as a separate PNG.");
                        ui.label("All pages from the input PDF will be extracted.");
                    }
                    PdfOperation::ExtractText => {
                        ui.label("Extracts all text from the PDF into a .txt file.");
                    }
                    PdfOperation::Merge => {
                        ui.label("Select at least 2 PDF files in the input panel.");
                        ui.label("They will be merged in order.");
                    }
                    PdfOperation::ImageToPdf => {
                        ui.label("Select one or more images.");
                        ui.label("They will be combined into a single PDF, one per page.");
                    }
                }
            });
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("OUTPUT").strong().size(14.0));
            ui.separator();

            ui.horizontal(|ui| {
                let dir_text = state.output_dir
                    .as_ref()
                    .map(|d| {
                        let p = std::path::Path::new(d);
                        p.file_name().and_then(|s| s.to_str()).unwrap_or(d)
                    })
                    .unwrap_or("(auto: same as input)")
                    .to_string();
                ui.label(format!("Folder: {dir_text}"));
                if ui.button("Browse…").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        let path = dir.to_string_lossy().to_string();
                        state.output_dir = Some(path);
                    }
                }
                if state.output_dir.is_some() {
                    if ui.small_button("×").clicked() {
                        state.output_dir = None;
                    }
                }
            });

            if state.output_dir.is_none() {
                ui.label(egui::RichText::new(format!("Preview: {}", state.default_output_name())).weak());
            }

            ui.add_space(8.0);

            // Execute button
            let can_run = state.can_execute();
            let run_label = if state.running {
                "⏳ Running…"
            } else {
                "▶ Execute"
            };
            if ui.add_enabled(can_run, egui::Button::new(
                egui::RichText::new(run_label).size(16.0),
            ).min_size(egui::vec2(ui.available_width(), 36.0)))
            .clicked()
            {
                execute_operation(state);
            }

            ui.add_space(8.0);
            ui.separator();
            ui.label(egui::RichText::new("LOG").strong().size(14.0));
            ui.separator();

            if ui.button("Clear Log").clicked() {
                state.clear_log();
            }

            egui::ScrollArea::vertical()
                .id_salt("toolbox_log")
                .max_height(ui.available_height().max(100.0))
                .show(ui, |ui| {
                    for entry in &state.log {
                        let color = if entry.is_error {
                            egui::Color32::from_rgb(200, 60, 60)
                        } else {
                            egui::Color32::from_rgb(60, 160, 60)
                        };
                        ui.label(egui::RichText::new(&entry.message).color(color));
                    }
                    if state.log.is_empty() {
                        ui.label(egui::RichText::new("No operations yet.").weak());
                    }
                });
        });
    });
}

fn execute_operation(state: &mut PdfToolboxState) {
    let output_dir = state.resolve_output_dir();
    state.log.push(LogEntry {
        message: format!("Starting: {} …", state.operation_name()),
        is_error: false,
    });

    let result = match state.operation {
        PdfOperation::Merge => {
            let out = format!("{}/{}", output_dir, state.default_output_name());
            pdf_operations::merge_pdfs(&state.input_files, &out)
                .map(|_| vec![out])
        }
        PdfOperation::Split => {
            let input = &state.input_files[0];
            match state.split_mode {
                SplitMode::Range => {
                    let out = format!("{}/{}", output_dir, state.default_output_name());
                    pdf_operations::split_pdf_by_range(input, &out, state.split_start, state.split_end)
                        .map(|_| vec![out])
                }
                SplitMode::EveryNPages => {
                    pdf_operations::split_pdf_every_n(input, &output_dir, state.split_every_n)
                }
                SplitMode::ByToc => {
                    pdf_operations::split_pdf_by_toc(input, &output_dir)
                }
            }
        }
        PdfOperation::ExtractImages => {
            let input = &state.input_files[0];
            let total = pdf_operations::page_count(input).unwrap_or(0);
            let pages: Vec<usize> = (0..total).collect();
            pdf_operations::extract_pages_as_images(input, &output_dir, &pages)
        }
        PdfOperation::ExtractText => {
            let input = &state.input_files[0];
            let out = format!("{}/{}", output_dir, state.default_output_name());
            pdf_operations::extract_pdf_text(input, &out)
                .map(|_| vec![out])
        }
        PdfOperation::ImageToPdf => {
            let out = format!("{}/{}", output_dir, state.default_output_name());
            pdf_operations::images_to_pdf(&state.input_files, &out)
                .map(|_| vec![out])
        }
    };

    match result {
        Ok(paths) => {
            state.log.push(LogEntry {
                message: format!("✓ Done. {} file(s) written.", paths.len()),
                is_error: false,
            });
            for p in &paths {
                state.log.push(LogEntry {
                    message: format!("  → {p}"),
                    is_error: false,
                });
            }
        }
        Err(e) => {
            state.log.push(LogEntry {
                message: format!("✗ Error: {e}"),
                is_error: true,
            });
        }
    }
}
