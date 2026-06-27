use crate::app::core::{AppState, Mode, document_manager::DocumentManager, mode_system::ModeController};
use crate::app::core::mode_system::ViewMode;
use crate::app::platform::font_loader::FontLoader;
use super::mode_ui;

pub struct FolixApp {
    pub state: AppState,
    pub open_dialog: bool,
    pub show_about: bool,
    pub status_message: String,
}

impl FolixApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::configure_fonts(&cc.egui_ctx);

        let mut app = Self {
            state: AppState::new(),
            open_dialog: false,
            show_about: false,
            status_message: String::new(),
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
}

impl eframe::App for FolixApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render_menu_bar(ctx);
        self.render_toolbar(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_document_view(ui);
        });

        self.render_status_bar(ctx);
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
                        self.state.document = None;
                        self.state.document_path = None;
                        self.status_message = "Closed document".to_string();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        std::process::exit(0);
                    }
                });

                ui.menu_button("Mode", |ui| {
                    let modes = ["Reading", "Auto", "Annotate"];
                    for mode_name in &modes {
                        let selected = self.state.mode.name() == *mode_name;
                        if ui.selectable_label(selected, *mode_name).clicked() {
                            self.state.switch(match *mode_name {
                                "Reading" => Mode::reading(),
                                "Auto" => Mode::auto(),
                                "Annotate" => Mode::annotate(),
                                _ => Mode::reading(),
                            });
                            ui.close_menu();
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

    fn render_toolbar(&mut self, ctx: &egui::Context) {
        let current_name = self.state.mode.name().to_string();
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Mode:");
                for name in ["Reading", "Auto", "Annotate"] {
                    let selected = current_name == name;
                    if ui.selectable_label(selected, name).clicked() && !selected {
                        self.state.switch(match name {
                            "Reading" => Mode::reading(),
                            "Auto" => Mode::auto(),
                            "Annotate" => Mode::annotate(),
                            _ => Mode::reading(),
                        });
                    }
                }
            });
        });
    }

    fn render_document_view(&mut self, ui: &mut egui::Ui) {
        let mode_name = self.state.mode.name().to_string();
        let pinned_names: Vec<String> = self.state.feature_system.pinned_features(&mode_name)
            .iter().map(|f| f.id.clone()).collect();
        let document = self.state.document.clone();

        ui.horizontal(|ui| {
            for name in &pinned_names {
                let _ = ui.button(format!("[{}]", name));
            }
        });

        match &mut self.state.mode {
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
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Mode: {}", self.state.mode.name()));
                ui.separator();
                if let Some(ref path) = self.state.document_path {
                    let name = std::path::Path::new(path)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    ui.label(format!("Document: {}", name));
                } else {
                    ui.label("No document open");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(&self.status_message);
                });
            });
        });
    }

    fn handle_open_dialog(&mut self, _ctx: &egui::Context) {
        if self.open_dialog {
            let path = rfd::FileDialog::new()
                .add_filter("Documents", &["pdf", "epub", "txt"])
                .pick_file();
            if let Some(path) = path {
                let path_str = path.to_string_lossy().to_string();
                if let Some(doc) = DocumentManager::open(&path_str) {
                    if let Mode::Reading(ref mut rs) = self.state.mode {
                        rs.page = 0;
                        rs.view_mode = if doc.lock().supports_image() {
                            ViewMode::Image
                        } else {
                            ViewMode::Text
                        };
                    }
                    self.state.document = Some(doc);
                    self.state.document_path = Some(path_str.clone());
                    self.state.feature_system.use_feature("open_file");
                    self.status_message = format!("Opened: {}", path_str);
                } else {
                    self.status_message = format!("Failed to open: {}", path_str);
                }
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
