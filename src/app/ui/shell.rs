use crate::app::core::{AppState, ModeKind, TabModes, ReadingLayout, document_manager::DocumentManager};
use crate::app::core::app_state::TabContent;
use crate::app::core::mode_system::{ViewMode, AutoPlayMode, Annotation, AnnotationTool, EditState, ContentEditState};
use crate::app::core::shortcuts::{ShortcutAction as SA, ALL_ACTIONS, AVAILABLE_KEYS};
use crate::app::engines::edit_operations;
use crate::app::paginator::Paginator;
use crate::app::platform::font_loader::FontLoader;
use crate::app::storage::sqlite::Database;
use super::mode_ui;
use std::collections::HashMap;

pub struct FolixApp {
    pub state: AppState,
    pub open_dialog: bool,
    pub show_about: bool,
    pub status_message: String,
    pub recent_files: Vec<String>,
    pub db: Option<Database>,
    pub image_texture_cache: HashMap<String, egui::TextureHandle>,
}

impl FolixApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::configure_fonts(&cc.egui_ctx);

        let db = Database::open("./folix.db").ok();

        let config = crate::app::config::ConfigData::load();
        let mut state = AppState::new();
        if let Some(ref cfg) = config {
            state.settings = cfg.settings.clone();
        }
        let recent_files = config.as_ref().map(|c| c.recent_files.clone()).unwrap_or_default();

        let mut app = Self {
            state,
            open_dialog: false,
            show_about: false,
            status_message: String::new(),
            recent_files,
            db,
            image_texture_cache: HashMap::new(),
        };
        app.init_features();
        app
    }

    fn sync_progress(&self) {
        if let Some(ref db) = self.db {
            if let Some(tab) = self.state.current_tab() {
                if let Some(ref book_id) = tab.book_id {
                    let _ = db.save_progress(book_id, tab.modes.page, tab.modes.auto.progress as f64);
                }
            }
        }
    }

    fn save_config(&self) {
        let data = crate::app::config::ConfigData {
            settings: self.state.settings.clone(),
            recent_files: self.recent_files.clone(),
        };
        data.save();
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
            ("open_file", "Light"),
            ("toggle_mode", "Light"),
            ("play_pause", "Light"),
            ("speed_control", "Light"),
            ("select_tool", "Deep"),
            ("undo", "Deep"),
        ];
        for (id, scope) in &features {
            self.state.feature_system.register(id, scope);
        }
    }

    fn open_file(&mut self, path_str: String) {
        self.image_texture_cache.clear();
        self.recent_files.retain(|p| p != &path_str);
        self.recent_files.insert(0, path_str.clone());
        self.recent_files.truncate(10);
        self.save_config();

        if let Some(doc) = DocumentManager::open(&path_str) {
            let replace = self.state.current_tab()
                .map(|t| t.is_new_tab())
                .unwrap_or(false);

            if replace {
                let idx = self.state.active_tab;
                let tab = &mut self.state.tabs[idx];
                tab.content = TabContent::Document;
                tab.document = Some(doc);
                tab.path = Some(path_str.clone());
                tab.modes = TabModes::new();
                tab.modes.reading.view_mode = if tab.document.as_ref().unwrap().lock().is_fixed() {
                    ViewMode::Image
                } else {
                    ViewMode::Text
                };
            } else {
                self.state.add_tab(path_str.clone(), doc);
            }

            // Sync with database: ensure book entry, set book_id, load annotations & progress
            if let Some(ref db) = self.db {
                if let Some(tab) = self.state.current_tab_mut() {
                    if let Some(ref d) = tab.document {
                        let title = d.lock().title();
                        let format = if path_str.to_lowercase().ends_with(".pdf") { "pdf" }
                            else if path_str.to_lowercase().ends_with(".epub") { "epub" }
                            else { "txt" };
                        if let Ok(book_id) = db.ensure_book(&path_str, &title, format) {
                            tab.book_id = Some(book_id.clone());
                            // Load annotations from DB
                            if let Ok(rows) = db.get_annotations(&book_id) {
                                for (_, page, kind_str, rect_data, note) in rows {
                                    let kind = match kind_str.as_str() {
                                        "Pen" => crate::app::core::mode_system::AnnotationTool::Pen,
                                        "Note" => crate::app::core::mode_system::AnnotationTool::Note,
                                        "Eraser" => crate::app::core::mode_system::AnnotationTool::Eraser,
                                        _ => crate::app::core::mode_system::AnnotationTool::Highlight,
                                    };
                                    let rect = rect_data.as_deref()
                                        .and_then(|s| serde_json::from_str::<[f32; 4]>(s).ok())
                                        .unwrap_or([0.0; 4]);
                                    tab.modes.annotate.annotations.push(Annotation {
                                        id: uuid::Uuid::new_v4().to_string(),
                                        doc_id: book_id.clone(),
                                        kind,
                                        page,
                                        rect,
                                        note,
                                        color: [255, 255, 0, 120],
                                    });
                                }
                            }
                            // Load progress
                            if let Ok(Some((saved_page, _))) = db.load_progress(&book_id) {
                                let is_fixed = d.lock().as_fixed().is_some();
                                if is_fixed {
                                    let max = d.lock().as_fixed()
                                        .map(|f| f.page_count().saturating_sub(1))
                                        .unwrap_or(0);
                                    tab.modes.page = saved_page.min(max);
                                } else {
                                    // Reflow: save for later (paginator not yet created)
                                    tab.modes.reading.stream_jump_to = Some(saved_page);
                                }
                            }
                        }
                    }
                }
            }

            // Ensure paginator for reflowable documents
            if let Some(tab) = self.state.current_tab_mut() {
                Self::ensure_paginator(tab);
                // If there's a pending jump (set before paginator existed), ensure
                // enough pages are loaded so the stream jump works first frame.
                if let Some(target) = tab.modes.reading.stream_jump_to {
                    let max = page_count_for_tab(tab).saturating_sub(1);
                    tab.modes.reading.stream_page_end = target.min(max);
                }
            }

            self.state.feature_system.use_feature("open_file");
            self.status_message = format!("Opened: {}", path_str);
        } else {
            self.status_message = format!("Failed to open: {}", path_str);
        }
    }

    fn ensure_paginator(tab: &mut crate::app::core::app_state::OpenTab) {
        if let Some(ref doc) = tab.document {
            let is_reflow = doc.lock().is_reflow();
            if is_reflow && tab.modes.paginator.is_none() {
                let chapters: Vec<(String, Vec<crate::app::engines::ContentBlock>)> = {
                    let handle = doc.lock();
                    let reflow = handle.as_reflow().unwrap();
                    let count = reflow.chapter_count();
                    (0..count).map(|i| {
                        let ch = reflow.load_chapter(i);
                        (ch.title, ch.blocks)
                    }).collect()
                };
                tab.modes.paginator = Some(Paginator::new(chapters, 800.0, 1000.0, 16.0));
            }
        }
    }

    fn reload_document(&mut self, path: &str) {
        if let Some(doc) = DocumentManager::open(path) {
            if let Some(tab) = self.state.current_tab_mut() {
                tab.document = Some(doc);
                // Recreate paginator for reflow docs
                Self::ensure_paginator(tab);
                if let Some(d) = &tab.document {
                    let count = d.lock().as_fixed().map(|f| f.page_count())
                        .or_else(|| tab.modes.paginator.as_ref().map(|p| p.page_count()))
                        .unwrap_or(0);
                    let max = count.saturating_sub(1);
                    tab.modes.page = tab.modes.page.min(max);
                    tab.modes.auto.progress = (tab.modes.auto.progress as usize).min(max) as f32;
                }
                self.status_message = format!("Saved: {}", path);
            }
        } else {
            self.status_message = format!("Failed to reload: {}", path);
        }
    }

    fn shortcut(&self, ctx: &egui::Context, action: SA) -> bool {
        self.state.settings.shortcuts.get(&action)
            .or_else(|| crate::app::core::shortcuts::DEFAULT_SHORTCUTS.get(&action))
            .map(|combo| combo.check(ctx))
            .unwrap_or(false)
    }

    fn apply_highlight_selection(tab: &mut crate::app::core::app_state::OpenTab) {
        let has_sel = !tab.modes.reading.selection.selected_word_indices.is_empty()
            && tab.modes.reading.selection.page == tab.modes.page;
        if has_sel {
            if let Some(ref doc) = tab.document {
                let page = tab.modes.page;
                let words = doc.lock().as_fixed().map(|f| f.page_text_positions(page)).unwrap_or_default();
                let indices = &tab.modes.reading.selection.selected_word_indices;
                let mut x0 = f32::MAX; let mut y0 = f32::MAX;
                let mut x1 = f32::MIN; let mut y1 = f32::MIN;
                for &idx in indices {
                    if let Some(w) = words.get(idx) {
                        x0 = x0.min(w.x0); y0 = y0.min(w.y0);
                        x1 = x1.max(w.x1); y1 = y1.max(w.y1);
                    }
                }
                if x0 != f32::MAX {
                    tab.modes.annotate.annotations.push(Annotation {
                        id: uuid::Uuid::new_v4().to_string(),
                        doc_id: String::new(),
                        kind: AnnotationTool::Highlight,
                        page,
                        rect: [x0, y0, x1, y1],
                        note: None,
                        color: tab.modes.annotate.current_color,
                    });
                    tab.modes.reading.selection.selected_word_indices.clear();
                    tab.modes.reading.selection.anchor = None;
                    tab.modes.reading.selection.focus = None;
                }
            }
        }
    }
}

impl eframe::App for FolixApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply dark/light theme
        ctx.set_visuals(if self.state.settings.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });

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

        // Tab toggles UI visibility (also available as configurable shortcut)
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)) {
            self.state.ui_visible = !self.state.ui_visible;
        }

        // Keyboard shortcuts (from config, with built-in defaults)
        if self.shortcut(ctx, SA::OpenFile) { self.open_dialog = true; }

        if self.shortcut(ctx, SA::CloseTab) {
            self.state.close_tab(self.state.active_tab);
            self.sync_progress();
        }

        if self.shortcut(ctx, SA::Quit) {
            self.sync_progress();
            std::process::exit(0);
        }

        if self.shortcut(ctx, SA::Reload) {
            if let Some(ref p) = self.state.current_tab().and_then(|t| t.path.clone()) {
                self.reload_document(&p);
            }
        }

        if self.shortcut(ctx, SA::ZoomIn) {
            if let Some(tab) = self.state.current_tab_mut() {
                tab.modes.scale = (tab.modes.scale + 0.1).min(3.0);
            }
        }

        if self.shortcut(ctx, SA::ZoomOut) {
            if let Some(tab) = self.state.current_tab_mut() {
                tab.modes.scale = (tab.modes.scale - 0.1).max(0.5);
            }
        }

        if self.shortcut(ctx, SA::PrevPage) {
            if let Some(tab) = self.state.current_tab_mut() {
                let cur = tab.modes.page;
                if cur > 0 { page_jump(tab, cur - 1); }
            }
        }

        if self.shortcut(ctx, SA::NextPage) {
            if let Some(tab) = self.state.current_tab_mut() {
                let cur = tab.modes.page;
                let max = page_count_for_tab(tab).saturating_sub(1);
                if cur < max { page_jump(tab, cur + 1); }
            }
        }

        if self.shortcut(ctx, SA::FirstPage) {
            if let Some(tab) = self.state.current_tab_mut() { page_jump(tab, 0); }
        }

        if self.shortcut(ctx, SA::LastPage) {
            if let Some(tab) = self.state.current_tab_mut() {
                let max = page_count_for_tab(tab).saturating_sub(1);
                page_jump(tab, max);
            }
        }

        if self.shortcut(ctx, SA::ScrollDown) {
            if let Some(tab) = self.state.current_tab_mut() {
                if tab.modes.reading_layout == ReadingLayout::Scroll {
                    tab.modes.reading.scroll_offset_y += 600.0;
                } else {
                    let max = page_count_for_tab(tab).saturating_sub(1);
                    let cur = tab.modes.page;
                    if cur < max { page_jump(tab, cur + 1); }
                }
            }
        }

        if self.shortcut(ctx, SA::ScrollUp) {
            if let Some(tab) = self.state.current_tab_mut() {
                if tab.modes.reading_layout == ReadingLayout::Scroll {
                    tab.modes.reading.scroll_offset_y = (tab.modes.reading.scroll_offset_y - 600.0).max(0.0);
                } else if tab.modes.page > 0 { page_jump(tab, tab.modes.page - 1); }
            }
        }

        if self.shortcut(ctx, SA::HighlightSel) {
            if let Some(tab) = self.state.current_tab_mut() {
                Self::apply_highlight_selection(tab);
            }
        }

        if self.shortcut(ctx, SA::AddBookmark) {
            if let Some(tab) = self.state.current_tab_mut() {
                tab.modes.reading.bookmarks.push(crate::app::core::mode_system::Bookmark {
                    page: tab.modes.page,
                    label: format!("Page {}", tab.modes.page + 1),
                });
            }
        }

        if self.shortcut(ctx, SA::ToggleSidebar) {
            if let Some(tab) = self.state.current_tab_mut() {
                tab.modes.reading.show_sidebar = !tab.modes.reading.show_sidebar;
            }
        }

        // Copy: only consume Ctrl+C for image-based docs (PDF).
        // For text-based docs (EPUB), let Label::selectable(true) handle it natively.
        let supports_image = self.state.current_tab()
            .and_then(|t| t.document.as_ref())
            .map(|d| d.lock().is_fixed())
            .unwrap_or(false);
        if supports_image && self.shortcut(ctx, SA::Copy) {
            if let Some(tab) = self.state.current_tab() {
                let sel = &tab.modes.reading.selection;
                if !sel.selected_word_indices.is_empty() {
                    if let Some(doc) = &tab.document {
                        let words = doc.lock().as_fixed().map(|f| f.page_text_positions(sel.page)).unwrap_or_default();
                        let text: String = sel.selected_word_indices.iter()
                            .filter_map(|&i| words.get(i))
                            .map(|w| w.text.as_str())
                            .collect::<Vec<&str>>().join(" ");
                        ctx.copy_text(text);
                    }
                }
            }
        }

        if self.state.ui_visible {
            self.render_menu_bar(ctx);
        }
        self.render_tab_bar(ctx);

        // Sidebar (LightReading & DeepReading only)
        let sidebar = self.state.current_tab().is_some_and(|t| {
            t.has_document() && (t.modes.active == ModeKind::LightReading || t.modes.active == ModeKind::DeepReading) && t.modes.reading.show_sidebar
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
                        let active = tab.modes.active;
                        if active == ModeKind::LightReading || active == ModeKind::DeepReading {
                            mode_ui::render_sidebar(ui, &doc, &mut tab.modes.page, &mut tab.modes.paginator, &mut tab.modes.reading);
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
            self.render_toolbars(ctx);
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
                    let is_fixed = self.state.current_tab()
                        .and_then(|t| t.document.as_ref())
                        .map(|d| d.lock().is_fixed())
                        .unwrap_or(true);
                    let mode_names: &[&str] = if is_fixed {
                        &["LightReading", "DeepReading", "PageEdit"]
                    } else {
                        &["LightReading", "DeepReading", "ContentEdit"]
                    };
                    let current_name = self.state.current_tab()
                        .map(|t| t.modes.active.name().to_string())
                        .unwrap_or_else(|| "Light".to_string());
                    for mode_name in mode_names {
                        let selected = current_name == *mode_name;
                        if ui.selectable_label(selected, *mode_name).clicked() {
                            if let Some(tab) = self.state.tabs.get_mut(self.state.active_tab) {
                                tab.modes.switch_to(match *mode_name {
                                    "DeepReading" => ModeKind::DeepReading,
                                    "PageEdit" => ModeKind::PageEdit,
                                    "ContentEdit" => ModeKind::ContentEdit,
                                    _ => ModeKind::LightReading,
                                });
                            }
                            ui.close_menu();
                        }
                    }

                    if current_name == "LightReading" {
                        ui.separator();
                        let layout = self.state.current_tab()
                            .map(|t| t.modes.reading_layout);
                        if let Some(layout) = layout {
                            if ui.selectable_label(layout == ReadingLayout::Paged, "Paged").clicked() {
                                if let Some(tab) = self.state.tabs.get_mut(self.state.active_tab) {
                                    tab.modes.reading_layout = ReadingLayout::Paged;
                                }
                                ui.close_menu();
                            }
                            if ui.selectable_label(layout == ReadingLayout::Scroll, "Scroll").clicked() {
                                if let Some(tab) = self.state.tabs.get_mut(self.state.active_tab) {
                                    tab.modes.reading_layout = ReadingLayout::Scroll;
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
                let has_doc = self.state.current_tab().map_or(false, |t| t.has_document());
                let show_side = self.state.current_tab().map_or(false, |t| {
                    let active = t.modes.active;
                    (active == ModeKind::LightReading || active == ModeKind::DeepReading) && t.modes.reading.show_sidebar
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

                // Settings button
                if ui.button("⚙").clicked() {
                    self.state.add_settings_tab();
                }

                // "+" button to create a new tab
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

    fn render_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(20.0);

        // ── Appearance ──
        ui.heading("Appearance");
        ui.separator();
        egui::Grid::new("appearance_grid").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
            ui.label("Toolbar Icon Size:");
            ui.add(egui::Slider::new(&mut self.state.settings.toolbar_icon_size, 12.0..=32.0));
            ui.end_row();

            ui.label("Background Color:");
            let mut color = [
                self.state.settings.background_color[0] as f32 / 255.0,
                self.state.settings.background_color[1] as f32 / 255.0,
                self.state.settings.background_color[2] as f32 / 255.0,
                self.state.settings.background_color[3] as f32 / 255.0,
            ];
            ui.color_edit_button_rgba_unmultiplied(&mut color);
            self.state.settings.background_color = [
                (color[0] * 255.0) as u8,
                (color[1] * 255.0) as u8,
                (color[2] * 255.0) as u8,
                (color[3] * 255.0) as u8,
            ];
            ui.end_row();
        });
        ui.checkbox(&mut self.state.settings.show_toolbar, "Show Toolbar");
        ui.checkbox(&mut self.state.settings.dark_mode, "Dark Mode (Night)");

        ui.add_space(20.0);

        // ── Keyboard Shortcuts ──
        ui.heading("Keyboard Shortcuts");
        ui.separator();
        ui.label("Click a shortcut row to edit its key binding.");
        ui.add_space(8.0);

        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            egui::Grid::new("shortcuts_grid").num_columns(5).spacing([12.0, 4.0]).striped(true).show(ui, |ui| {
                ui.strong("Action");
                ui.strong("Key");
                ui.strong("Ctrl");
                ui.strong("Shift");
                ui.strong("Alt");
                ui.end_row();

                let s = &mut self.state.settings;
                for (i, action) in ALL_ACTIONS.iter().enumerate() {
                    let combo = s.shortcuts.get_mut(action);

                    ui.label(action.label());

                    if s.editing_shortcut == Some(i) {
                        if let Some(combo) = combo {
                            let mut key_idx = AVAILABLE_KEYS.iter().position(|k| *k == combo.key).unwrap_or(0);
                            egui::ComboBox::from_id_salt(format!("key_{}", i))
                                .selected_text(&combo.key)
                                .show_ui(ui, |ui| {
                                    for (j, k) in AVAILABLE_KEYS.iter().enumerate() {
                                        ui.selectable_value(&mut key_idx, j, *k);
                                    }
                                });
                            if key_idx < AVAILABLE_KEYS.len() {
                                combo.key = AVAILABLE_KEYS[key_idx].to_string();
                            }
                            ui.checkbox(&mut combo.ctrl, "");
                            ui.checkbox(&mut combo.shift, "");
                            ui.checkbox(&mut combo.alt, "");
                        } else {
                            ui.label("(unset)");
                            ui.label(""); ui.label(""); ui.label("");
                        }
                        if ui.button("Done").clicked() {
                            s.editing_shortcut = None;
                        }
                    } else {
                        if let Some(combo) = combo {
                            if ui.button(combo.display()).clicked() {
                                s.editing_shortcut = Some(i);
                            }
                        } else {
                            ui.label("(unset)");
                        }
                        ui.label(""); ui.label(""); ui.label("");
                        ui.label("");
                    }
                    ui.end_row();
                }
            });
        });

        ui.add_space(16.0);

        // ── Info ──
        ui.heading("Info");
        ui.separator();
        ui.label("Config file: ./folix.conf");
        ui.horizontal(|ui| {
            if ui.button("Reset Shortcuts to Default").clicked() {
                self.state.settings.shortcuts = crate::app::core::shortcuts::default_shortcuts();
                self.state.settings.editing_shortcut = None;
            }
            if ui.button("Save Config Now").clicked() {
                self.save_config();
                self.status_message = "Config saved".to_string();
            }
        });

        self.save_config();
    }

    fn render_document_view(&mut self, ui: &mut egui::Ui) {
        let idx = self.state.active_tab;

        // New tab page
        if self.state.tabs[idx].is_new_tab() {
            self.render_new_tab_page(ui);
            return;
        }

        // Settings tab
        if self.state.tabs[idx].is_settings_tab() {
            self.render_settings_tab(ui);
            return;
        }

        // Document tab
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
        let ctx = ui.ctx().clone();
        let is_light = tab.modes.active == ModeKind::LightReading;
        let is_deep = tab.modes.active == ModeKind::DeepReading;
        let dark_mode = self.state.settings.dark_mode;
        // Ensure paginator exists for reflow docs
        Self::ensure_paginator(tab);
        mode_ui::render_document(
            ui, &document,
            &mut tab.modes.page,
            &mut tab.modes.scale,
            &mut tab.modes.reading_layout,
            &mut tab.modes.paginator,
            &mut tab.modes.reading,
            if is_light { Some(&mut tab.modes.auto) } else { None },
            if is_deep { Some(&mut tab.modes.annotate) } else { None },
            Some(ctx),
            dark_mode,
            &mut self.image_texture_cache,
        );

        // Sync annotations to database
        if is_deep {
            if let Some(ref db) = self.db {
                if let Some(book_id) = &tab.book_id {
                    let _ = db.delete_book_annotations(book_id);
                    for ann in &tab.modes.annotate.annotations {
                        let kind_str = format!("{:?}", ann.kind);
                        let rect_str = serde_json::to_string(&ann.rect).ok();
                        let _ = db.add_annotation(
                            book_id, ann.page, &kind_str,
                            rect_str.as_deref(),
                            ann.note.as_deref(),
                        );
                    }
                }
            }
        }
    }

    fn render_toolbars(&mut self, ctx: &egui::Context) {
        if !self.state.settings.show_toolbar {
            return;
        }

        let mut needs_reload: Option<String> = None;

        egui::TopBottomPanel::bottom("toolbar_row1").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let tab = self.state.current_tab_mut();
                if tab.is_none() { return; }
                let tab = tab.unwrap();

                let doc_count = page_count_for_tab(tab);

                // Ensure active mode is compatible with document type
                let is_fixed_doc = tab.document.as_ref().map(|d| d.lock().is_fixed()).unwrap_or(true);
                let valid_for_fixed = matches!(tab.modes.active, ModeKind::LightReading | ModeKind::DeepReading | ModeKind::PageEdit);
                let valid_for_reflow = matches!(tab.modes.active, ModeKind::LightReading | ModeKind::DeepReading | ModeKind::ContentEdit);
                if (is_fixed_doc && !valid_for_fixed) || (!is_fixed_doc && !valid_for_reflow) {
                    tab.modes.active = ModeKind::LightReading;
                }
                let mode_names: &[ModeKind] = if is_fixed_doc {
                    &[ModeKind::LightReading, ModeKind::DeepReading, ModeKind::PageEdit]
                } else {
                    &[ModeKind::LightReading, ModeKind::DeepReading, ModeKind::ContentEdit]
                };
                for &mk in mode_names {
                    let selected = tab.modes.active == mk;
                    if ui.selectable_label(selected, mk.name()).clicked() {
                        tab.modes.switch_to(mk);
                    }
                }
                ui.separator();

                if doc_count > 0 {
                    // Prev/Next
                    let is_paged = tab.modes.reading_layout == ReadingLayout::Paged;
                    if is_paged || tab.modes.active == ModeKind::DeepReading {
                        if ui.add_enabled(tab.modes.page > 0, egui::Button::new("◀")).clicked() {
                            page_jump(tab, tab.modes.page.saturating_sub(1));
                        }
                        if ui.add_enabled(tab.modes.page + 1 < doc_count, egui::Button::new("▶")).clicked() {
                            page_jump(tab, tab.modes.page + 1);
                        }
                        ui.separator();
                    }

                    // Zoom
                    ui.label("🔍");
                    let z = tab.modes.scale;
                    if ui.add_enabled(z > 0.5, egui::Button::new("−")).clicked() {
                        tab.modes.scale = (z - 0.1).max(0.5);
                    }
                    let mut new_scale = tab.modes.scale;
                    ui.add(egui::Slider::new(&mut new_scale, 0.5..=3.0).text("×"));
                    if (new_scale - tab.modes.scale).abs() > 0.001 {
                        tab.modes.scale = new_scale;
                    }
                    if ui.add_enabled(z < 3.0, egui::Button::new("+")).clicked() {
                        tab.modes.scale = (z + 0.1).min(3.0);
                    }
                    ui.separator();

                    // Layout toggle
                    let layout_label = if is_paged { "Paged" } else { "Scroll" };
                    if ui.button(layout_label).clicked() {
                        tab.modes.reading_layout = if is_paged { ReadingLayout::Scroll } else { ReadingLayout::Paged };
                    }
                    ui.separator();

                    // Page number on the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("Page {}/{}", tab.modes.page + 1, doc_count));
                    });
                }
            });
        });

        egui::TopBottomPanel::bottom("toolbar_row2").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let tab = self.state.current_tab_mut();
                if tab.is_none() { return; }
                let tab = tab.unwrap();

                let doc_count = page_count_for_tab(tab);

                if doc_count == 0 { return; }

                // Row 2: Mode-specific controls
                match tab.modes.active {
                    ModeKind::LightReading => {
                        let play_label = if tab.modes.auto.playing { "⏸" } else { "▶" };
                        if ui.button(play_label).clicked() {
                            tab.modes.auto.playing = !tab.modes.auto.playing;
                            if tab.modes.auto.playing {
                                tab.modes.auto.progress = 0.0;
                            }
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
                    ModeKind::DeepReading => {
                        if tab.modes.active == ModeKind::DeepReading {
                            let tool = &tab.modes.annotate.tool;
                            let is_sel = *tool == AnnotationTool::Highlight;
                            let is_pen = *tool == AnnotationTool::Pen;
                            let is_eraser = *tool == AnnotationTool::Eraser;
                            if ui.selectable_label(is_sel, "Sel").clicked() {
                                tab.modes.annotate.tool = AnnotationTool::Highlight;
                            }
                            if ui.selectable_label(is_pen, "Pen").clicked() {
                                tab.modes.annotate.tool = AnnotationTool::Pen;
                            }
                            if ui.selectable_label(is_eraser, "Eraser").clicked() {
                                tab.modes.annotate.tool = AnnotationTool::Eraser;
                            }
                            ui.separator();

                            // Highlight Selected button
                            let has_sel = !tab.modes.reading.selection.selected_word_indices.is_empty()
                                && tab.modes.reading.selection.page == tab.modes.page;
                            if ui.add_enabled(has_sel, egui::Button::new("High")).clicked() {
                                if let Some(ref doc) = tab.document {
                                    let page = tab.modes.page;
                                    let words = doc.lock().as_fixed().map(|f| f.page_text_positions(page)).unwrap_or_default();
                                    let indices = &tab.modes.reading.selection.selected_word_indices;
                                    let mut x0 = f32::MAX; let mut y0 = f32::MAX;
                                    let mut x1 = f32::MIN; let mut y1 = f32::MIN;
                                    for &idx in indices {
                                        if let Some(w) = words.get(idx) {
                                            x0 = x0.min(w.x0); y0 = y0.min(w.y0);
                                            x1 = x1.max(w.x1); y1 = y1.max(w.y1);
                                        }
                                    }
                                    if x0 != f32::MAX {
                                        tab.modes.annotate.annotations.push(Annotation {
                                            id: uuid::Uuid::new_v4().to_string(),
                                            doc_id: String::new(),
                                            kind: AnnotationTool::Highlight,
                                            page,
                                            rect: [x0, y0, x1, y1],
                                            note: None,
                                            color: tab.modes.annotate.current_color,
                                        });
                                        tab.modes.reading.selection.selected_word_indices.clear();
                                        tab.modes.reading.selection.anchor = None;
                                        tab.modes.reading.selection.focus = None;
                                    }
                                }
                            }

                            // Note on last highlight button
                            if ui.button("Note").clicked() {
                                let page = tab.modes.page;
                                if let Some(last) = tab.modes.annotate.annotations.iter().rev().find(|a| {
                                    a.page == page && a.kind == AnnotationTool::Highlight
                                }) {
                                    tab.modes.annotate.editing_note_id = Some(last.id.clone());
                                    tab.modes.annotate.note_text_buffer = last.note.clone().unwrap_or_default();
                                }
                            }

                            ui.separator();
                            // Color swatches
                            for &c in &crate::app::core::mode_system::HIGHLIGHT_COLORS {
                                let c32 = egui::Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]);
                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                                let fill = if tab.modes.annotate.current_color == c { c32 } else { c32.gamma_multiply(0.6) };
                                ui.painter().rect_filled(rect, 3.0, fill);
                                if resp.clicked() {
                                    tab.modes.annotate.current_color = c;
                                }
                            }
                            ui.separator();
                            if ui.button("Undo").clicked() {
                                tab.modes.annotate.annotations.pop();
                            }
                            if ui.button("Clr").clicked() {
                                tab.modes.annotate.annotations.clear();
                            }
                        }
                    }
                    ModeKind::PageEdit => {
                        let path = tab.path.clone();
                        if let Some(ref p) = path {
                            if ui.button("↻ CW").clicked() {
                                let page = tab.modes.page;
                                if edit_operations::rotate_page(p, page, 90).is_ok() {
                                    needs_reload = Some(p.clone());
                                }
                            }
                            if ui.button("↻ CCW").clicked() {
                                let page = tab.modes.page;
                                if edit_operations::rotate_page(p, page, 270).is_ok() {
                                    needs_reload = Some(p.clone());
                                }
                            }
                            if ui.button("Del").clicked() {
                                let page = tab.modes.page;
                                if doc_count > 1 {
                                    if edit_operations::delete_page(p, page).is_ok() {
                                        needs_reload = Some(p.clone());
                                    }
                                }
                            }
                            if ui.button("+ Page").clicked() {
                                let page = tab.modes.page;
                                if edit_operations::insert_blank_page(p, page).is_ok() {
                                    needs_reload = Some(p.clone());
                                }
                            }
                        }
                    }
                    ModeKind::ContentEdit => {
                        if !matches!(tab.modes.edit, EditState::Content(_)) {
                            tab.modes.edit = EditState::Content(ContentEditState {
                                font_size_scale: 1.0, bold: false, italic: false,
                            });
                        }
                        let state = tab.modes.edit.as_content().unwrap();
                        if ui.button("A-").clicked() {
                            state.font_size_scale = (state.font_size_scale - 0.1).max(0.5);
                        }
                        if ui.button("A+").clicked() {
                            state.font_size_scale = (state.font_size_scale + 0.1).min(2.0);
                        }
                        ui.label(format!("{:.0}%", state.font_size_scale * 100.0));
                        if ui.selectable_label(state.bold, "B").clicked() {
                            state.bold = !state.bold;
                        }
                        if ui.selectable_label(state.italic, "I").clicked() {
                            state.italic = !state.italic;
                        }
                    }
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

fn page_count_for_tab(tab: &crate::app::core::app_state::OpenTab) -> usize {
    if let Some(ref doc) = tab.document {
        let handle = doc.lock();
        if let Some(fixed) = handle.as_fixed() {
            fixed.page_count()
        } else if let Some(ref pag) = tab.modes.paginator {
            pag.page_count()
        } else {
            0
        }
    } else {
        0
    }
}

/// Navigate to a page, handling both fixed (PDF) and reflow (stream) documents.
fn page_jump(tab: &mut crate::app::core::app_state::OpenTab, target: usize) {
    let max = page_count_for_tab(tab).saturating_sub(1);
    let target = target.min(max);
    if tab.modes.paginator.is_some() {
        // For reflow: set stream jump target + ensure enough pages loaded
        tab.modes.reading.stream_jump_to = Some(target);
        tab.modes.reading.stream_page_end = tab.modes.reading.stream_page_end.max(target);
    }
    tab.modes.page = target;
}
