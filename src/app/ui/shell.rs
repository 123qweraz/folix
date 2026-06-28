use crate::app::core::{AppState, Mode, ReadingLayout, document_manager::DocumentManager};
use crate::app::core::mode_system::ViewMode;
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
                tab.mode = Mode::reading();
                if let Mode::Reading(ref mut rs) = tab.mode {
                    rs.view_mode = if tab.document.as_ref().unwrap().lock().supports_image() {
                        ViewMode::Image
                    } else {
                        ViewMode::Text
                    };
                }
            } else {
                self.state.add_tab(path_str.clone(), doc);
            }
            self.state.feature_system.use_feature("open_file");
            self.status_message = format!("Opened: {}", path_str);
        } else {
            self.status_message = format!("Failed to open: {}", path_str);
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
        let sidebar = self.state.current_tab().map_or(false, |t| {
            t.document.is_some() && matches!(t.mode, Mode::Reading(ref rs) if rs.show_sidebar)
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
                        if let Mode::Reading(ref mut rs) = tab.mode {
                            let total = doc.lock().page_count();
                            mode_ui::render_sidebar(ui, &doc, rs, total);
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
                        .map(|t| t.mode.name().to_string())
                        .unwrap_or_else(|| "Reading".to_string());
                    for mode_name in &modes {
                        let selected = current_name == *mode_name;
                        if ui.selectable_label(selected, *mode_name).clicked() {
                            if let Some(tab) = self.state.tabs.get_mut(self.state.active_tab) {
                                tab.mode = match *mode_name {
                                    "Auto" => Mode::auto(),
                                    "Annotate" => Mode::annotate(),
                                    _ => Mode::reading(),
                                };
                            }
                            ui.close_menu();
                        }
                    }

                    if current_name == "Reading" {
                        ui.separator();
                        let layout = self.state.current_tab()
                            .and_then(|t| {
                                if let Mode::Reading(ref rs) = t.mode {
                                    Some(rs.reading_layout)
                                } else {
                                    None
                                }
                            });
                        if let Some(layout) = layout {
                            if ui.selectable_label(layout == ReadingLayout::Paged, "Paged").clicked() {
                                if let Some(tab) = self.state.tabs.get_mut(self.state.active_tab) {
                                    if let Mode::Reading(ref mut rs) = tab.mode {
                                        rs.reading_layout = ReadingLayout::Paged;
                                    }
                                }
                                ui.close_menu();
                            }
                            if ui.selectable_label(layout == ReadingLayout::Scroll, "Scroll").clicked() {
                                if let Some(tab) = self.state.tabs.get_mut(self.state.active_tab) {
                                    if let Mode::Reading(ref mut rs) = tab.mode {
                                        rs.reading_layout = ReadingLayout::Scroll;
                                    }
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
                let show_side = self.state.current_tab().and_then(|t| {
                    if let Mode::Reading(ref rs) = t.mode { Some(rs.show_sidebar) } else { None }
                }).unwrap_or(false);
                let side_btn = if show_side { "📑 Sidebar" } else { "📑" };
                if has_doc {
                    if ui.button(side_btn).clicked() {
                        if let Some(t) = self.state.current_tab_mut() {
                            if let Mode::Reading(ref mut rs) = t.mode {
                                rs.show_sidebar = !show_side;
                            }
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

        let mode_name = self.state.tabs[idx].mode.name().to_string();
        let pinned_names: Vec<String> = self.state.feature_system.pinned_features(&mode_name)
            .iter().map(|f| f.id.clone()).collect();

        ui.horizontal(|ui| {
            for name in &pinned_names {
                let _ = ui.button(format!("[{}]", name));
            }
        });

        let document = self.state.tabs[idx].document.as_ref().unwrap().clone();
        let tab = &mut self.state.tabs[idx];
        match &mut tab.mode {
            Mode::Reading(ref mut rs) => {
                mode_ui::render_reading(ui, &document, rs);
            }
            Mode::Auto(ref mut aut) => {
                let ctx = ui.ctx().clone();
                mode_ui::render_auto(ui, &document, aut, ctx);
            }
            Mode::Annotate(ref mut an) => {
                mode_ui::render_annotate(ui, &document, an);
            }
        }
    }

    fn render_status_bar(&mut self, ctx: &egui::Context) {
        let doc_count = self.state.current_tab()
            .and_then(|t| t.document.as_ref().map(|d| d.lock().page_count()))
            .unwrap_or(0);

        let reading_info = self.state.current_tab().and_then(|t| {
            if let Mode::Reading(ref rs) = t.mode {
                Some((rs.page, rs.scale, rs.reading_layout))
            } else {
                None
            }
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some((page, scale, layout)) = reading_info {
                    let is_paged = layout == ReadingLayout::Paged;

                    if is_paged && doc_count > 0 {
                        let prev_enabled = page > 0;
                        if ui.add_enabled(prev_enabled, egui::Button::new("◀ Prev")).clicked() {
                            if let Some(t) = self.state.current_tab_mut() {
                                if let Mode::Reading(ref mut r) = t.mode {
                                    r.page -= 1;
                                }
                            }
                        }
                    }

                    ui.label(format!("Page {}/{}", page + 1, doc_count));

                    if is_paged && doc_count > 0 {
                        let next_enabled = page + 1 < doc_count;
                        if ui.add_enabled(next_enabled, egui::Button::new("Next ▶")).clicked() {
                            if let Some(t) = self.state.current_tab_mut() {
                                if let Mode::Reading(ref mut r) = t.mode {
                                    r.page += 1;
                                }
                            }
                        }
                    }

                    ui.separator();
                    ui.label("Zoom:");
                    let mut new_scale = scale;
                    ui.add(egui::Slider::new(&mut new_scale, 0.5..=3.0).text("x"));
                    if (new_scale - scale).abs() > 0.001 {
                        if let Some(t) = self.state.current_tab_mut() {
                            if let Mode::Reading(ref mut r) = t.mode {
                                r.scale = new_scale;
                            }
                        }
                    }

                    ui.separator();
                    let layout_label = if is_paged { "Paged" } else { "Scroll" };
                    if ui.button(layout_label).clicked() {
                        if let Some(t) = self.state.current_tab_mut() {
                            if let Mode::Reading(ref mut r) = t.mode {
                                r.reading_layout = if is_paged { ReadingLayout::Scroll } else { ReadingLayout::Paged };
                            }
                        }
                    }
                } else {
                    let name = self.state.current_tab()
                        .map(|t| t.mode.name())
                        .unwrap_or("N/A");
                    ui.label(format!("Mode: {}", name));
                }
            });
        });
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
