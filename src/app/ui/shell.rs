use crate::app::core::{AppState, ModeKind, TabModes, ReadingLayout, document_manager::DocumentManager};
use crate::app::core::mode_system::{ViewMode, AnnotationTool, AutoPlayMode};
use crate::app::engines::edit_operations;
use crate::app::platform::font_loader::FontLoader;
use super::mode_ui;

pub struct FolixApp {
    pub state: AppState,
    pub open_dialog: bool,
    pub show_about: bool,
    pub status_message: String,
    pub recent_files: Vec<String>,
}

impl FolixApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::configure_fonts(&cc.egui_ctx);

        let mut app = Self {
            state: AppState::new(),
            open_dialog: false,
            show_about: false,
            status_message: String::new(),
            recent_files: Vec::new(),
        };
        app.init_features();
        app
    }

    fn configure_fonts(ctx: &egui::Context) {
        let loader = FontLoader::new();
        let font_paths = loader.load_system_fonts();
        if font_paths.is_empty() {
            return;
        }

        let mut fonts = egui::FontDefinitions::default();

        for path in &font_paths {
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            let name = format!("cjk_{}", ext);
            match std::fs::read(path) {
                Ok(data) => {
                    let index = if ext == "ttc" { 2 } else { 0 };
                    let mut font_data = egui::FontData::from_owned(data);
                    font_data.index = index;
                    fonts.font_data.insert(name.clone(), std::sync::Arc::new(font_data));

                    for family in &[egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                        let list = fonts.families.get_mut(family).unwrap();
                        if !list.contains(&name) {
                            list.insert(0, name.clone());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load font {:?}: {}", path, e);
                }
            }
        }

        ctx.set_fonts(fonts);
    }

    fn init_features(&mut self) {
        let features = [
            ("open_file", "Reading"),
            ("save_progress", "Reading"),
            ("toggle_mode", "Reading"),
            ("play_pause", "Auto"),
            ("speed_control", "Auto"),
            ("select_tool", "Annotate"),
            ("undo", "Annotate"),
        ];
        for (id, scope) in &features {
            self.state.feature_system.register(id, scope);
        }
    }

    fn open_file(&mut self, path_str: String) {
        self.recent_files.retain(|p| p != &path_str);
        self.recent_files.insert(0, path_str.clone());
        self.recent_files.truncate(10);

        if let Some(doc) = DocumentManager::open(&path_str) {
            let replace = self.state.current_tab()
                .map(|t| t.is_new_tab())
                .unwrap_or(false);

            if replace {
                let idx = self.state.active_tab;
                let tab = &mut self.state.tabs[idx];
                tab.document = Some(doc);
                tab.path = Some(path_str.clone());
                tab.modes = TabModes::new();
                tab.modes.reading.view_mode = if tab.document.as_ref().unwrap().lock().supports_image() {
                    ViewMode::Image
                } else {
                    ViewMode::Text
                };
            } else {
                self.state.add_tab(path_str.clone(), doc);
            }
            self.state.feature_system.use_feature("open_file");
            self.status_message = format!("Opened: {}", path_str);
        } else {
            self.status_message = format!("Failed to open: {}", path_str);
        }
    }

    fn reload_document(&mut self, path: &str) {
        if let Some(doc) = DocumentManager::open(path) {
            if let Some(tab) = self.state.current_tab_mut() {
                tab.document = Some(doc);
                if let Some(d) = &tab.document {
                    let count = d.lock().page_count();
                    let max = count.saturating_sub(1);
                    tab.modes.annotate.page = tab.modes.annotate.page.min(max);
                    tab.modes.reading.page = tab.modes.reading.page.min(max);
                    tab.modes.edit.page = tab.modes.edit.page.min(max);
                    tab.modes.auto.progress = (tab.modes.auto.progress as usize).min(max) as f32;
                }
                self.status_message = format!("Saved: {}", path);
            }
        } else {
            self.status_message = format!("Failed to reload: {}", path);
        }
    }
}

impl eframe::App for FolixApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle dropped files
        let dropped_files: Vec<String> = ctx.input(|i| {
            i.raw.dropped_files.iter()
                .filter_map(|f| f.path.as_ref())
                .map(|p| p.to_string_lossy().to_string())
                .collect()
        });
        for path in dropped_files {
            self.open_file(path);
        }

        // Tab toggles UI visibility
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)) {
            self.state.ui_visible = !self.state.ui_visible;
        }

        if self.state.ui_visible {
            self.render_menu_bar(ctx);
        }
        self.render_tab_bar(ctx);

        // Sidebar (Reading mode only)
        let sidebar = self.state.current_tab().is_some_and(|t| {
            t.document.is_some() && t.modes.active == ModeKind::Reading && t.modes.reading.show_sidebar
        });
        if sidebar {
            let doc = self.state.current_tab()
                .and_then(|t| t.document.clone())
                .unwrap();
            egui::SidePanel::left("reading_sidebar")
                .resizable(true)
                .default_width(260.0)
                .show(ctx, |ui| {
                    if let Some(tab) = self.state.current_tab_mut() {
                        if tab.modes.active == ModeKind::Reading {
                            let total = doc.lock().page_count();
                            mode_ui::render_sidebar(ui, &doc, &mut tab.modes.reading, total);
                        }
                    }
                });
        }

        let panel_resp = egui::CentralPanel::default().show(ctx, |ui| {
            self.render_document_view(ui);
        });

        // Left-click on the document panel toggles UI visibility
        if panel_resp.response.clicked() {
            self.state.ui_visible = !self.state.ui_visible;
        }

        if self.state.ui_visible {
            self.render_status_bar(ctx);
        }
        self.handle_open_dialog(ctx);
        self.render_about(ctx);
    }
}

impl FolixApp {
    fn render_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open...").clicked() {
                        self.open_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button("Close").clicked() {
                        if !self.state.tabs.is_empty() {
                            self.state.close_tab(self.state.active_tab);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        std::process::exit(0);
                    }
                });

                ui.menu_button("Mode", |ui| {
                    let modes = ["Reading", "Auto", "Annotate"];
                    let current_name = self.state.current_tab()
                        .map(|t| t.modes.active.name().to_string())
                        .unwrap_or_else(|| "Reading".to_string());
                    for mode_name in &modes {
                        let selected = current_name == *mode_name;
                        if ui.selectable_label(selected, *mode_name).clicked() {
                            if let Some(tab) = self.state.tabs.get_mut(self.state.active_tab) {
                                tab.modes.switch_to(match *mode_name {
                                    "Auto" => ModeKind::Auto,
                                    "Annotate" => ModeKind::Annotate,
                                    _ => ModeKind::Reading,
                                });
                            }
                            ui.close_menu();
                        }
                    }

                    if current_name == "Reading" {
                        ui.separator();
                        let layout = self.state.current_tab()
                            .map(|t| t.modes.reading.reading_layout);
                        if let Some(layout) = layout {
                            if ui.selectable_label(layout == ReadingLayout::Paged, "Paged").clicked() {
                                if let Some(tab) = self.state.tabs.get_mut(self.state.active_tab) {
                                    tab.modes.reading.reading_layout = ReadingLayout::Paged;
                                }
                                ui.close_menu();
                            }
                            if ui.selectable_label(layout == ReadingLayout::Scroll, "Scroll").clicked() {
                                if let Some(tab) = self.state.tabs.get_mut(self.state.active_tab) {
                                    tab.modes.reading.reading_layout = ReadingLayout::Scroll;
                                }
                                ui.close_menu();
                            }
                        }
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About Folix").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn render_tab_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Sidebar toggle — leftmost position
                let has_doc = self.state.current_tab().map_or(false, |t| t.document.is_some());
                let show_side = self.state.current_tab().map_or(false, |t| {
                    t.modes.active == ModeKind::Reading && t.modes.reading.show_sidebar
                });
                let side_btn = if show_side { "📑 Sidebar" } else { "📑" };
                if has_doc {
                    if ui.button(side_btn).clicked() {
                        if let Some(t) = self.state.current_tab_mut() {
                            t.modes.reading.show_sidebar = !show_side;
                        }
                    }
                    ui.separator();
                }

                // "+" button to create a new tab page
                if ui.button(" + ").clicked() {
                    self.state.add_new_tab();
                }

                let mut to_close: Option<usize> = None;
                let mut i = 0;
                while i < self.state.tabs.len() {
                    let title = self.state.tabs[i].title();
                    let is_active = i == self.state.active_tab;

                    if ui.selectable_label(is_active, &title).clicked() {
                        self.state.active_tab = i;
                    }

                    if ui.button("×").clicked() {
                        to_close = Some(i);
                    }

                    i += 1;
                }

                if let Some(idx) = to_close {
                    self.state.close_tab(idx);
                }
            });
        });
    }

    fn render_new_tab_page(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(100.0);
            ui.heading("Folix");
            ui.label("PDF / EPUB / TXT Reader");
            ui.add_space(20.0);
            if ui.add(egui::Button::new("📂  Open File").min_size(egui::vec2(200.0, 36.0))).clicked() {
                self.open_dialog = true;
            }
            ui.add_space(24.0);

            if !self.recent_files.is_empty() {
                ui.label("Recent Files");
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for path in self.recent_files.clone() {
                            let name = std::path::Path::new(&path)
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or(&path);
                            if ui.selectable_label(false, name).clicked() {
                                self.open_file(path);
                            }
                        }
                    });
            } else {
                ui.label("No recent files");
                ui.colored_label(egui::Color32::GRAY, "Open a file or drag-and-drop to get started.");
            }
        });
    }

    fn render_document_view(&mut self, ui: &mut egui::Ui) {
        let idx = self.state.active_tab;

        // New tab page
        if self.state.tabs[idx].is_new_tab() {
            self.render_new_tab_page(ui);
            return;
        }

        let mode_name = self.state.tabs[idx].modes.active.name().to_string();
        let pinned_names: Vec<String> = self.state.feature_system.pinned_features(&mode_name)
            .iter().map(|f| f.id.clone()).collect();

        ui.horizontal(|ui| {
            for name in &pinned_names {
                let _ = ui.button(format!("[{}]", name));
            }
        });

        let document = self.state.tabs[idx].document.as_ref().unwrap().clone();
        let tab = &mut self.state.tabs[idx];
        match tab.modes.active {
            ModeKind::Reading => {
                mode_ui::render_reading(ui, &document, &mut tab.modes.reading);
            }
            ModeKind::Auto => {
                let ctx = ui.ctx().clone();
                mode_ui::render_auto(ui, &document, &mut tab.modes.auto, ctx);
            }
            ModeKind::Annotate => {
                mode_ui::render_annotate(ui, &document, &mut tab.modes.annotate);
            }
            ModeKind::Edit => {
                mode_ui::render_edit(ui, &document, &mut tab.modes.edit);
            }
        }
    }

    fn render_status_bar(&mut self, ctx: &egui::Context) {
        let mut needs_reload: Option<String> = None;

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let tab = self.state.current_tab_mut();
                if tab.is_none() { return; }
                let tab = tab.unwrap();

                let doc_count = tab.document.as_ref()
                    .map(|d| d.lock().page_count())
                    .unwrap_or(0);

                // Mode tabs — always visible
                let mode_names = [ModeKind::Reading, ModeKind::Auto, ModeKind::Annotate, ModeKind::Edit];
                for &mk in &mode_names {
                    let selected = tab.modes.active == mk;
                    if ui.selectable_label(selected, mk.name()).clicked() {
                        tab.modes.switch_to(mk);
                    }
                }
                ui.separator();

                // Mode-specific controls
                match tab.modes.active {
                    ModeKind::Reading => {
                        let is_paged = tab.modes.reading.reading_layout == ReadingLayout::Paged;
                        if doc_count > 0 {
                            if is_paged {
                                if ui.add_enabled(tab.modes.reading.page > 0, egui::Button::new("◀ Prev")).clicked() {
                                    tab.modes.reading.page -= 1;
                                }
                                if ui.add_enabled(tab.modes.reading.page + 1 < doc_count, egui::Button::new("Next ▶")).clicked() {
                                    tab.modes.reading.page += 1;
                                }
                                ui.separator();
                            }
                            ui.label("Zoom:");
                            let mut new_scale = tab.modes.reading.scale;
                            ui.add(egui::Slider::new(&mut new_scale, 0.5..=3.0).text("x"));
                            if (new_scale - tab.modes.reading.scale).abs() > 0.001 {
                                tab.modes.reading.scale = new_scale;
                            }
                            ui.separator();
                            let layout_label = if is_paged { "Paged" } else { "Scroll" };
                            if ui.button(layout_label).clicked() {
                                tab.modes.reading.reading_layout = if is_paged { ReadingLayout::Scroll } else { ReadingLayout::Paged };
                            }
                        }
                    }
                    ModeKind::Auto => {
                        if doc_count > 0 {
                            let play_label = if tab.modes.auto.playing { "⏸" } else { "▶" };
                            if ui.button(play_label).clicked() {
                                tab.modes.auto.playing = !tab.modes.auto.playing;
                            }
                            ui.label("Speed:");
                            ui.add(egui::Slider::new(&mut tab.modes.auto.speed, 0.5..=5.0).text("x"));
                            ui.separator();
                            ui.label("Mode:");
                            egui::ComboBox::from_id_salt("auto_mode")
                                .selected_text(format!("{:?}", tab.modes.auto.auto_mode))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut tab.modes.auto.auto_mode, AutoPlayMode::PageFlow, "Page Flow");
                                    ui.selectable_value(&mut tab.modes.auto.auto_mode, AutoPlayMode::GlyphReveal, "Glyph Reveal");
                                    ui.selectable_value(&mut tab.modes.auto.auto_mode, AutoPlayMode::SentenceStream, "Sentence Stream");
                                });
                        }
                    }
                    ModeKind::Annotate => {
                        if doc_count > 0 {
                            let tools = [
                                (AnnotationTool::Highlight, "🖊  High"),
                                (AnnotationTool::Pen, "✏  Pen"),
                                (AnnotationTool::Note, "📝  Note"),
                                (AnnotationTool::Eraser, "🧹  Eraser"),
                                (AnnotationTool::Select, "👆  Select"),
                            ];
                            for (tool, label) in &tools {
                                let is_selected = std::mem::discriminant(&tab.modes.annotate.tool) == std::mem::discriminant(tool);
                                if ui.selectable_label(is_selected, *label).clicked() {
                                    tab.modes.annotate.tool = tool.clone();
                                }
                            }
                            ui.separator();
                            if ui.button("Undo").clicked() {
                                tab.modes.annotate.annotations.pop();
                            }
                            if ui.button("Clr").clicked() {
                                tab.modes.annotate.annotations.clear();
                            }

                            let supports_image = tab.document.as_ref()
                                .map(|d| d.lock().supports_image())
                                .unwrap_or(false);
                            if supports_image {
                                ui.separator();
                                if ui.add_enabled(tab.modes.annotate.page > 0, egui::Button::new("◀")).clicked() {
                                    tab.modes.annotate.page -= 1;
                                }
                                if ui.add_enabled(tab.modes.annotate.page + 1 < doc_count, egui::Button::new("▶")).clicked() {
                                    tab.modes.annotate.page += 1;
                                }
                            }
                        }
                    }
                    ModeKind::Edit => {
                        if doc_count > 0 {
                            let supports_image = tab.document.as_ref()
                                .map(|d| d.lock().supports_image())
                                .unwrap_or(false);
                            if supports_image {
                                if ui.add_enabled(tab.modes.edit.page > 0, egui::Button::new("◀")).clicked() {
                                    tab.modes.edit.page -= 1;
                                }
                                if ui.add_enabled(tab.modes.edit.page + 1 < doc_count, egui::Button::new("▶")).clicked() {
                                    tab.modes.edit.page += 1;
                                }
                                ui.separator();
                                let path = tab.path.clone();
                                if let Some(ref p) = path {
                                    if ui.button("↻ CW").clicked() {
                                        let page = tab.modes.edit.page;
                                        if edit_operations::rotate_page(p, page, 90).is_ok() {
                                            needs_reload = Some(p.clone());
                                        }
                                    }
                                    if ui.button("↻ CCW").clicked() {
                                        let page = tab.modes.edit.page;
                                        if edit_operations::rotate_page(p, page, 270).is_ok() {
                                            needs_reload = Some(p.clone());
                                        }
                                    }
                                    if ui.button("Del").clicked() {
                                        let page = tab.modes.edit.page;
                                        if doc_count > 1 {
                                            if edit_operations::delete_page(p, page).is_ok() {
                                                needs_reload = Some(p.clone());
                                            }
                                        }
                                    }
                                    if ui.button("+ Page").clicked() {
                                        let page = tab.modes.edit.page;
                                        if edit_operations::insert_blank_page(p, page).is_ok() {
                                            needs_reload = Some(p.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Page number on the right
                if doc_count > 0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        match tab.modes.active {
                            ModeKind::Reading => {
                                ui.label(format!("Page {}/{}", tab.modes.reading.page + 1, doc_count));
                            }
                            ModeKind::Auto => {
                                let cp = tab.modes.auto.progress as usize;
                                ui.label(format!("Page {}/{}", cp + 1, doc_count));
                            }
                            ModeKind::Annotate => {
                                ui.label(format!("Page {}/{}", tab.modes.annotate.page + 1, doc_count));
                            }
                            ModeKind::Edit => {
                                ui.label(format!("Page {}/{}", tab.modes.edit.page + 1, doc_count));
                            }
                        }
                    });
                }
            });
        });

        if let Some(path) = needs_reload {
            self.reload_document(&path);
        }
    }

    fn handle_open_dialog(&mut self, _ctx: &egui::Context) {
        if self.open_dialog {
            let path = rfd::FileDialog::new()
                .add_filter("Documents", &["pdf", "epub", "txt"])
                .pick_file();
            if let Some(path) = path {
                self.open_file(path.to_string_lossy().to_string());
            }
            self.open_dialog = false;
        }
    }

    fn render_about(&mut self, ctx: &egui::Context) {
        if self.show_about {
            egui::Window::new("About Folix")
                .open(&mut self.show_about)
                .show(ctx, |ui| {
                    ui.heading("Folix");
                    ui.label("PDF/EPUB Reader v0.1.0");
                    ui.separator();
                    ui.label("A document reading engine with mode state machine + GPU rendering.");
                    ui.label("Stack: Rust, egui, wgpu, MuPDF, SQLite FTS5");
                });
        }
    }
}
