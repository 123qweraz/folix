use crate::app::core::{AppState, ModeKind, TabModes, ReadingLayout, document_manager::DocumentManager};
use crate::app::core::app_state::TabContent;
use crate::app::core::mode_system::{ViewMode, FitMode, ViewRotation, Annotation, AnnotationTool, EditState, ContentEditState, Bookmark, Vocabulary, Sentence_};
use crate::app::config::RecentFile;
use crate::app::core::shortcuts::{key_from_str, ShortcutAction as SA, ALL_ACTIONS, AVAILABLE_KEYS};
use crate::app::engines::edit_operations;
use crate::app::paginator::Paginator;
use crate::app::platform::font_loader::FontLoader;
use crate::app::storage::sqlite::Database;
use super::{mode_ui, pdf_toolbox};
use std::collections::HashMap;

pub struct FolixApp {
    pub state: AppState,
    pub open_dialog: bool,
    pub show_about: bool,
    pub status_message: String,
    pub recent_files: Vec<RecentFile>,
    pub db: Option<Database>,
    pub image_texture_cache: HashMap<String, egui::TextureHandle>,
    pub settings_section: usize,
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
        let recent_files: Vec<RecentFile> = config.as_ref().map(|c| c.recent_files.clone()).unwrap_or_default();

        let mut app = Self {
            state,
            open_dialog: false,
            show_about: false,
            status_message: String::new(),
            recent_files,
            db,
            image_texture_cache: HashMap::new(),
            settings_section: 0,
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
            recent_files: self.recent_files.iter().map(|f| RecentFile {
                path: f.path.clone(),
                pinned: f.pinned,
            }).collect(),
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
        let lng_s = self.state.settings.language.clone();
        let lng = &lng_s;
        self.image_texture_cache.clear();
        // Update recent files: remove old entry, add to front (pinned preserved)
        let was_pinned = self.recent_files.iter()
            .find(|f| f.path == path_str)
            .map(|f| f.pinned)
            .unwrap_or(false);
        self.recent_files.retain(|f| f.path != path_str);
        self.recent_files.insert(0, RecentFile { path: path_str.clone(), pinned: was_pinned });
        // Truncate non-pinned files to keep max 10
        let pinned_count = self.recent_files.iter().filter(|f| f.pinned).count();
        let non_pinned_max = 10usize.saturating_sub(pinned_count);
        let mut non_pinned = 0usize;
        self.recent_files.retain(|f| {
            if f.pinned { true } else { non_pinned += 1; non_pinned <= non_pinned_max }
        });
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
                            // Load bookmarks
                            if let Ok(rows) = db.list_bookmarks(&book_id) {
                                for (_, page, label) in rows {
                                    tab.modes.reading.bookmarks.push(Bookmark {
                                        page,
                                        label: label.unwrap_or_default(),
                                    });
                                }
                            }
                            // Load vocabulary
                            if let Ok(rows) = db.list_vocabulary(&book_id) {
                                for (id, word, context, definition, page) in rows {
                                    tab.modes.reading.vocab.push(Vocabulary {
                                        id,
                                        word,
                                        context_sentence: context,
                                        definition,
                                        page,
                                    });
                                }
                            }
                            // Load sentences
                            if let Ok(rows) = db.list_sentences(&book_id) {
                                for (id, text, page) in rows {
                                    tab.modes.reading.sentences.push(Sentence_ {
                                        id,
                                        text,
                                        page,
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
            self.status_message = format!("{} {}", crate::app::i18n::tr(lng, "Opened:"), path_str);
        } else {
            self.status_message = format!("{} {}", crate::app::i18n::tr(lng, "Failed to open:"), path_str);
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
                tab.modes.reading.chapter_cache.clear();
            }
        }
    }

    fn reload_document(&mut self, path: &str) {
        let lng_s = self.state.settings.language.clone();
        let lng = &lng_s;
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
                self.status_message = format!("{} {}", crate::app::i18n::tr(lng, "Saved:"), path);
            }
        } else {
            self.status_message = format!("{} {}", crate::app::i18n::tr(lng, "Failed to reload:"), path);
        }
    }

    fn shortcut(&self, ctx: &egui::Context, action: SA) -> bool {
        self.state.settings.shortcuts.get(&action)
            .or_else(|| crate::app::core::shortcuts::DEFAULT_SHORTCUTS.get(&action))
            .map(|combo| combo.check(ctx))
            .unwrap_or(false)
    }

    /// Check if a shortcut key is currently held down (for continuous scroll).
    fn key_held(&self, ctx: &egui::Context, action: SA) -> bool {
        let combo = self.state.settings.shortcuts.get(&action)
            .or_else(|| crate::app::core::shortcuts::DEFAULT_SHORTCUTS.get(&action));
        if let Some(combo) = combo {
            if let Some(ekey) = key_from_str(&combo.key) {
                ctx.input(|i| i.key_down(ekey))
            } else {
                false
            }
        } else {
            false
        }
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
                    tab.modes.annotate.dirty = true;
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
        let lng_s = self.state.settings.language.clone();
        let lng = &lng_s;
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

        if self.shortcut(ctx, SA::NewTab) {
            self.state.add_new_tab();
        }

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

        // Arrow keys: scroll step in scroll mode only
        let speed = self.state.settings.scroll_speed;
        let arr_dn = ctx.input(|i| i.key_down(egui::Key::ArrowDown));
        let arr_up = ctx.input(|i| i.key_down(egui::Key::ArrowUp));
        if arr_dn || arr_up {
            if let Some(tab) = self.state.current_tab_mut() {
                if tab.modes.reading_layout == ReadingLayout::Scroll {
                    tab.modes.reading.scroll_velocity = if arr_dn { speed } else { -speed };
                }
            }
        }

        // Space / Shift+Space: scroll step or page turn
        let space_dn = self.key_held(ctx, SA::ScrollDown);
        let space_up = self.key_held(ctx, SA::ScrollUp);
        if space_dn || space_up {
            if let Some(tab) = self.state.current_tab_mut() {
                if tab.modes.reading_layout == ReadingLayout::Scroll {
                    tab.modes.reading.scroll_velocity = if space_dn { speed } else { -speed };
                } else {
                    let max = page_count_for_tab(tab).saturating_sub(1);
                    let cur = tab.modes.page;
                    if space_dn && cur < max { page_jump(tab, cur + 1); }
                    else if space_up && cur > 0 { page_jump(tab, cur - 1); }
                }
            }
        }

        if self.shortcut(ctx, SA::ScrollDown) {
            if let Some(tab) = self.state.current_tab_mut() {
                if tab.modes.reading_layout == ReadingLayout::Scroll {
                    tab.modes.reading.scroll_velocity = speed;
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
                    tab.modes.reading.scroll_velocity = -speed;
                } else if tab.modes.page > 0 { page_jump(tab, tab.modes.page - 1); }
            }
        }

        if self.shortcut(ctx, SA::ToggleAutoPlay) {
            if let Some(tab) = self.state.current_tab_mut() {
                if tab.modes.active == ModeKind::LightReading {
                    tab.modes.auto.playing = !tab.modes.auto.playing;
                    if tab.modes.auto.playing {
                        tab.modes.auto.progress = 0.0;
                    }
                }
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
                    label: format!("{} {}", crate::app::i18n::tr(lng, "Page"), tab.modes.page + 1),
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

        self.render_menu_bar(ctx);
        if self.state.ui_visible {
            self.render_tab_bar(ctx);
        }

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
                            mode_ui::render_sidebar(ui, &doc, &mut tab.modes.page, &mut tab.modes.paginator, &mut tab.modes.reading, lng);
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

        // Goto page dialog
        if let Some(tab) = self.state.current_tab_mut() {
            if tab.modes.reading.show_goto_dialog {
                let max = page_count_for_tab(tab).saturating_sub(1);
                let mut keep = true;
                egui::Window::new(crate::app::i18n::tr(lng, "Go to Page"))
                    .open(&mut keep)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(format!("{} (1-{})", crate::app::i18n::tr(lng, "Page number:"), max + 1));
                        ui.add(egui::TextEdit::singleline(&mut tab.modes.reading.goto_page_text)
                            .desired_width(100.0));
                        ui.add_space(8.0);
                        if ui.button(crate::app::i18n::tr(lng, "Go")).clicked() {
                            let target = tab.modes.reading.goto_page_text.trim().parse::<usize>().ok();
                            if let Some(p) = target {
                                let p = p.max(1).min(max + 1).saturating_sub(1);
                                page_jump(tab, p);
                            }
                            tab.modes.reading.show_goto_dialog = false;
                            tab.modes.reading.goto_page_text.clear();
                        }
                    });
                if !keep {
                    tab.modes.reading.show_goto_dialog = false;
                    tab.modes.reading.goto_page_text.clear();
                }
            }
        }
    }
}

impl FolixApp {
    fn render_menu_bar(&mut self, ctx: &egui::Context) {
        let lng_s = self.state.settings.language.clone();
        let lng = &lng_s;
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                // ── File ──
                ui.menu_button(crate::app::i18n::tr(lng, "File"), |ui| {
                    if ui.button(crate::app::i18n::tr(lng, "Open...")).clicked() {
                        self.open_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button(crate::app::i18n::tr(lng, "Close")).clicked() {
                        if !self.state.tabs.is_empty() {
                            self.state.close_tab(self.state.active_tab);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(crate::app::i18n::tr(lng, "Quit")).clicked() {
                        std::process::exit(0);
                    }
                });

                // ── Navigate ──
                ui.menu_button(crate::app::i18n::tr(lng, "Navigate"), |ui| {
                    if ui.button(crate::app::i18n::tr(lng, "Go to Page...")).clicked() {
                        if let Some(tab) = self.state.current_tab_mut() {
                            tab.modes.reading.show_goto_dialog = true;
                            tab.modes.reading.goto_page_text.clear();
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(crate::app::i18n::tr(lng, "First Page")).clicked() {
                        if let Some(tab) = self.state.current_tab_mut() { page_jump(tab, 0); }
                        ui.close_menu();
                    }
                    if ui.button(crate::app::i18n::tr(lng, "Last Page")).clicked() {
                        if let Some(tab) = self.state.current_tab_mut() {
                            let max = page_count_for_tab(tab).saturating_sub(1);
                            page_jump(tab, max);
                        }
                        ui.close_menu();
                    }
                    if ui.button(crate::app::i18n::tr(lng, "Prev Page")).clicked() {
                        if let Some(tab) = self.state.current_tab_mut() {
                            let cur = tab.modes.page;
                            if cur > 0 { page_jump(tab, cur - 1); }
                        }
                        ui.close_menu();
                    }
                    if ui.button(crate::app::i18n::tr(lng, "Next Page")).clicked() {
                        if let Some(tab) = self.state.current_tab_mut() {
                            let cur = tab.modes.page;
                            let max = page_count_for_tab(tab).saturating_sub(1);
                            if cur < max { page_jump(tab, cur + 1); }
                        }
                        ui.close_menu();
                    }
                });

                // ── Mode ──
                ui.menu_button(crate::app::i18n::tr(lng, "Mode"), |ui| {
                    let is_fixed = self.state.current_tab()
                        .and_then(|t| t.document.as_ref())
                        .map(|d| d.lock().is_fixed())
                        .unwrap_or(true);
                    let mode_kinds: &[ModeKind] = if is_fixed {
                        &[ModeKind::LightReading, ModeKind::DeepReading, ModeKind::PageEdit]
                    } else {
                        &[ModeKind::LightReading, ModeKind::DeepReading, ModeKind::ContentEdit]
                    };
                    let current = self.state.current_tab().map(|t| t.modes.active);
                    for &mk in mode_kinds {
                        let selected = current == Some(mk);
                        let label = mk.name(lng);
                        if ui.selectable_label(selected, label).clicked() {
                            if let Some(tab) = self.state.tabs.get_mut(self.state.active_tab) {
                                tab.modes.switch_to(mk);
                            }
                            ui.close_menu();
                        }
                    }

                    // Layout toggle (available in reading modes)
                    if current == Some(ModeKind::LightReading) || current == Some(ModeKind::DeepReading) {
                        ui.separator();
                        let layout = self.state.current_tab()
                            .map(|t| t.modes.reading_layout);
                        if let Some(layout) = layout {
                            if ui.selectable_label(layout == ReadingLayout::Paged, crate::app::i18n::tr(lng, "Paged")).clicked() {
                                if let Some(tab) = self.state.tabs.get_mut(self.state.active_tab) {
                                    tab.modes.reading_layout = ReadingLayout::Paged;
                                }
                                ui.close_menu();
                            }
                            if ui.selectable_label(layout == ReadingLayout::Scroll, crate::app::i18n::tr(lng, "Scroll")).clicked() {
                                if let Some(tab) = self.state.tabs.get_mut(self.state.active_tab) {
                                    tab.modes.reading_layout = ReadingLayout::Scroll;
                                }
                                ui.close_menu();
                            }
                        }
                    }

                    // Zoom controls
                    ui.separator();
                    if ui.button(crate::app::i18n::tr(lng, "Zoom In")).clicked() {
                        if let Some(tab) = self.state.current_tab_mut() {
                            tab.modes.scale = (tab.modes.scale + 0.1).min(3.0);
                        }
                        ui.close_menu();
                    }
                    if ui.button(crate::app::i18n::tr(lng, "Zoom Out")).clicked() {
                        if let Some(tab) = self.state.current_tab_mut() {
                            tab.modes.scale = (tab.modes.scale - 0.1).max(0.5);
                        }
                        ui.close_menu();
                    }
                });

                // ── Tools ──
                ui.menu_button(crate::app::i18n::tr(lng, "Tools"), |ui| {
                    let current = self.state.current_tab().map(|t| t.modes.active);

                    // Sidebar toggle
                    if ui.button(crate::app::i18n::tr(lng, "Toggle Sidebar")).clicked() {
                        if let Some(tab) = self.state.current_tab_mut() {
                            tab.modes.reading.show_sidebar = !tab.modes.reading.show_sidebar;
                        }
                        ui.close_menu();
                    }

                    // Add bookmark
                    if ui.button(crate::app::i18n::tr(lng, "Add Bookmark")).clicked() {
                        if let Some(tab) = self.state.current_tab_mut() {
                            tab.modes.reading.bookmarks.push(Bookmark {
                                page: tab.modes.page,
                                label: format!("{} {}", crate::app::i18n::tr(lng, "Page"), tab.modes.page + 1),
                            });
                            tab.modes.reading.bookmarks_dirty = true;
                        }
                        ui.close_menu();
                    }

                    // Deep-reading annotation tools
                    if current == Some(ModeKind::DeepReading) {
                        ui.separator();
                        let tool = self.state.current_tab().map(|t| t.modes.annotate.tool.clone());

                        if ui.selectable_label(tool == Some(AnnotationTool::Highlight), crate::app::i18n::tr(lng, "Sel")).clicked() {
                            if let Some(tab) = self.state.current_tab_mut() { tab.modes.annotate.tool = AnnotationTool::Highlight; }
                            ui.close_menu();
                        }
                        if ui.selectable_label(tool == Some(AnnotationTool::Pen), crate::app::i18n::tr(lng, "Pen")).clicked() {
                            if let Some(tab) = self.state.current_tab_mut() { tab.modes.annotate.tool = AnnotationTool::Pen; }
                            ui.close_menu();
                        }
                        if ui.selectable_label(tool == Some(AnnotationTool::Eraser), crate::app::i18n::tr(lng, "Eraser")).clicked() {
                            if let Some(tab) = self.state.current_tab_mut() { tab.modes.annotate.tool = AnnotationTool::Eraser; }
                            ui.close_menu();
                        }

                        if ui.button(crate::app::i18n::tr(lng, "High")).clicked() {
                            Self::apply_highlight_selection(
                                self.state.current_tab_mut().unwrap()
                            );
                            ui.close_menu();
                        }
                        if ui.button(crate::app::i18n::tr(lng, "Undo")).clicked() {
                            if let Some(tab) = self.state.current_tab_mut() {
                                tab.modes.annotate.annotations.pop();
                                tab.modes.annotate.dirty = true;
                            }
                            ui.close_menu();
                        }
                        if ui.button(crate::app::i18n::tr(lng, "Clr")).clicked() {
                            if let Some(tab) = self.state.current_tab_mut() {
                                tab.modes.annotate.annotations.clear();
                                tab.modes.annotate.dirty = true;
                            }
                            ui.close_menu();
                        }
                    }

                    // Light-reading auto-play
                    if current == Some(ModeKind::LightReading) {
                        ui.separator();
                        let playing = self.state.current_tab().map(|t| t.modes.auto.playing).unwrap_or(false);
                        let play_label = if playing { "⏸" } else { "▶" };
                        if ui.button(play_label).clicked() {
                            if let Some(tab) = self.state.current_tab_mut() {
                                tab.modes.auto.playing = !tab.modes.auto.playing;
                                if tab.modes.auto.playing {
                                    tab.modes.auto.progress = 0.0;
                                }
                            }
                            ui.close_menu();
                        }
                    }
                });

                // ── Help ──
                ui.menu_button(crate::app::i18n::tr(lng, "Help"), |ui| {
                    if ui.button(crate::app::i18n::tr(lng, "About Folix")).clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn render_tab_bar(&mut self, ctx: &egui::Context) {
        let lng_s = self.state.settings.language.clone();
        let lng = &lng_s;
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Sidebar toggle — leftmost position
                let has_doc = self.state.current_tab().map_or(false, |t| t.has_document());
                let show_side = self.state.current_tab().map_or(false, |t| {
                    let active = t.modes.active;
                    (active == ModeKind::LightReading || active == ModeKind::DeepReading) && t.modes.reading.show_sidebar
                });
                let side_btn = if show_side {
                    crate::app::i18n::tr(lng, "📑 Sidebar")
                } else {
                    "📑"
                };
                if has_doc {
                    if ui.button(side_btn).clicked() {
                        if let Some(t) = self.state.current_tab_mut() {
                            t.modes.reading.show_sidebar = !show_side;
                        }
                    }
                    ui.separator();
                }

                let mut to_close: Option<usize> = None;
                const TAB_W: f32 = 150.0;
                const TAB_H: f32 = 28.0;
                let style = ctx.style();
                let mut i = 0;
                while i < self.state.tabs.len() {
                    let title = self.state.tabs[i].title(lng);
                    let is_active = i == self.state.active_tab;

                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(TAB_W, TAB_H),
                        egui::Sense::click(),
                    );
                    let tab_resp = ui.interact(rect, ui.next_auto_id(), egui::Sense::click());
                    if tab_resp.clicked() {
                        self.state.active_tab = i;
                    }
                    if tab_resp.middle_clicked() {
                        to_close = Some(i);
                    }
                    // Paint background with active/hover/ inactive states
                    let bg = if is_active {
                        egui::Color32::WHITE
                    } else if tab_resp.hovered() {
                        style.visuals.faint_bg_color
                    } else {
                        egui::Color32::from_black_alpha(10)
                    };
                    ui.painter().rect_filled(rect, egui::CornerRadius::same(4), bg);
                    // Underline accent for active tab
                    if is_active {
                        let line_y = rect.bottom() - 2.0;
                        ui.painter().line_segment(
                            [egui::pos2(rect.left() + 4.0, line_y), egui::pos2(rect.right() - 4.0, line_y)],
                            egui::Stroke::new(2.0, style.visuals.selection.stroke.color),
                        );
                    }
                    // Content: title + ×
                    let inner = rect.shrink2(egui::vec2(6.0, 2.0));
                    let mut cui = ui.new_child(egui::UiBuilder::new()
                        .max_rect(inner)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)));
                    cui.add(
                        egui::Label::new(
                            egui::RichText::new(&title).size(13.0)
                        )
                        .truncate()
                        .selectable(false)
                    );
                    let x_resp = cui.add(
                        egui::Label::new(
                            egui::RichText::new("×").size(13.0).color(style.visuals.text_color()),
                        )
                        .sense(egui::Sense::click()),
                    );
                    if x_resp.clicked() {
                        to_close = Some(i);
                    }

                    i += 1;
                }

                // "+" button — after all tabs
                if ui.button(" + ").clicked() {
                    self.state.add_new_tab();
                }

                if let Some(idx) = to_close {
                    self.state.close_tab(idx);
                }
            });
        });
    }

    fn render_new_tab_page(&mut self, ui: &mut egui::Ui) {
        let lng_s = self.state.settings.language.clone();
        let lng = &lng_s;
        ui.vertical_centered(|ui| {
            ui.add_space(100.0);
            ui.heading(crate::app::i18n::tr(lng, "Folix"));
            ui.label(crate::app::i18n::tr(lng, "PDF / EPUB / TXT Reader"));
            ui.add_space(20.0);
            if ui.add(egui::Button::new(crate::app::i18n::tr(lng, "📂  Open File")).min_size(egui::vec2(200.0, 36.0))).clicked() {
                self.open_dialog = true;
            }
            ui.add_space(8.0);
            if ui.add(egui::Button::new(crate::app::i18n::tr(lng, "📄  PDF Tools")).min_size(egui::vec2(200.0, 36.0))).clicked() {
                self.state.add_pdf_toolbox_tab();
            }
            ui.add_space(8.0);
            if ui.add(egui::Button::new(crate::app::i18n::tr(lng, "⚙  Settings")).min_size(egui::vec2(200.0, 36.0))).clicked() {
                self.state.add_settings_tab();
            }
            ui.add_space(24.0);

            if !self.recent_files.is_empty() {
                ui.label(crate::app::i18n::tr(lng, "Recent Files"));
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        let mut to_remove: Option<usize> = None;
                        let mut to_toggle_pin: Option<usize> = None;

                        // Build sorted list: pinned first
                        let mut sorted: Vec<(usize, RecentFile)> = self.recent_files.iter()
                            .enumerate()
                            .map(|(i, f)| (i, f.clone()))
                            .collect();
                        sorted.sort_by(|a, b| {
                            b.1.pinned.cmp(&a.1.pinned)
                                .then_with(|| a.1.path.cmp(&b.1.path))
                        });

                        for (idx, rf) in &sorted {
                            let path = std::path::Path::new(&rf.path);
                            let name = path.file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or(&rf.path);
                            let parent = path.parent()
                                .and_then(|p| p.to_str())
                                .unwrap_or("");
                            let ext = path.extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_lowercase();
                            let icon = match ext.as_str() {
                                "pdf" => "📕",
                                "epub" => "📘",
                                "md" | "markdown" => "📝",
                                "docx" | "doc" => "📄",
                                "txt" => "📃",
                                "png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp" | "tiff" | "tif" => "🖼",
                                _ => "📄",
                            };

                            // Row with fixed-width frame
                            egui::Frame::NONE
                                .fill(egui::Color32::from_black_alpha(8))
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .corner_radius(egui::CornerRadius::same(4))
                                .show(ui, |ui| {
                                    ui.set_max_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        // Clickable area: icon + name + path
                                        let file_btn = ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(format!("{} {}", icon, name)).size(14.0)
                                            ).sense(egui::Sense::click())
                                        );
                                        if file_btn.clicked() {
                                            self.open_file(rf.path.clone());
                                        }
                                        if file_btn.middle_clicked() {
                                            // Show in folder on middle click
                                            show_in_folder(&rf.path);
                                        }
                                        // Path in gray
                                        if !parent.is_empty() {
                                            ui.colored_label(
                                                egui::Color32::GRAY,
                                                egui::RichText::new(parent).size(11.0),
                                            );
                                        }
                                        // Spacer + actions
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button("✕").clicked() {
                                                to_remove = Some(*idx);
                                            }
                                            if ui.button("📁").clicked() {
                                                show_in_folder(&rf.path);
                                            }
                                            let pin_icon = if rf.pinned { "📌" } else { "📍" };
                                            if ui.button(pin_icon).clicked() {
                                                to_toggle_pin = Some(*idx);
                                            }
                                        });
                                    });
                                });
                            ui.separator();
                        }

                        if let Some(idx) = to_remove {
                            // Find actual index in original vec
                            if let Some(pos) = self.recent_files.iter().position(|f| f.path == sorted[idx].1.path) {
                                self.recent_files.remove(pos);
                            }
                            self.save_config();
                        }
                        if let Some(idx) = to_toggle_pin {
                            if let Some(f) = self.recent_files.iter_mut().find(|f| f.path == sorted[idx].1.path) {
                                f.pinned = !f.pinned;
                            }
                            self.recent_files.sort_by(|a, b| b.pinned.cmp(&a.pinned).then_with(|| a.path.cmp(&b.path)));
                            self.save_config();
                        }
                    });
            } else {
                ui.label(crate::app::i18n::tr(lng, "No recent files"));
                ui.colored_label(egui::Color32::GRAY, crate::app::i18n::tr(lng, "Open a file or drag-and-drop to get started."));
            }
        });
    }

    fn render_settings_tab(&mut self, ui: &mut egui::Ui) {
        let lng_s = self.state.settings.language.clone();
        let lng = &lng_s;

        // Sidebar state
        const SETTINGS_SECTIONS: &[&str] = &["Appearance", "Scrolling", "Shortcuts", "Info"];
        let section_names: Vec<&str> = SETTINGS_SECTIONS.iter().map(|s| crate::app::i18n::tr(lng, s)).collect();
        let mut current = self.settings_section;

        egui::SidePanel::left("settings_sidebar")
            .resizable(false)
            .default_width(140.0)
            .show_inside(ui, |ui| {
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    for (i, name) in section_names.iter().enumerate() {
                        let selected = i == current;
                        if ui.selectable_label(selected, *name).clicked() {
                            current = i;
                        }
                    }
                });
            });

        self.settings_section = current;

        // Right content area with scroll
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);

            match self.settings_section {
                0 => {
                    // ── Appearance ──
                    ui.heading(crate::app::i18n::tr(lng, "Appearance"));
                    ui.separator();
                    egui::Grid::new("appearance_grid").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                        ui.label(crate::app::i18n::tr(lng, "Toolbar Icon Size:"));
                        ui.add(egui::Slider::new(&mut self.state.settings.toolbar_icon_size, 12.0..=32.0));
                        ui.end_row();

                        ui.label(crate::app::i18n::tr(lng, "Background Color:"));
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
                    ui.add_space(8.0);

                    ui.label(crate::app::i18n::tr(lng, "Language"));
                    egui::ComboBox::from_id_salt("lang_selector")
                        .selected_text({
                            if self.state.settings.language == "zh-CN" { "简体中文" } else { "English" }
                        })
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(self.state.settings.language == "zh-CN", "简体中文").clicked() {
                                self.state.settings.language = "zh-CN".into();
                                self.save_config();
                            }
                            if ui.selectable_label(self.state.settings.language == "en", "English").clicked() {
                                self.state.settings.language = "en".into();
                                self.save_config();
                            }
                        });
                    ui.add_space(8.0);

                    ui.label(crate::app::i18n::tr(lng, "Toolbars:"));
                    egui::Grid::new("toolbar_grid").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                        ui.checkbox(&mut self.state.settings.show_toolbar_nav, crate::app::i18n::tr(lng, "📖 Nav  ◀▶ ▲▼"));
                        ui.end_row();
                        ui.checkbox(&mut self.state.settings.show_toolbar_view, crate::app::i18n::tr(lng, "🔍 View  Zoom+Layout"));
                        ui.end_row();
                        ui.checkbox(&mut self.state.settings.show_toolbar_page, crate::app::i18n::tr(lng, "📄 Page"));
                        ui.end_row();
                        ui.checkbox(&mut self.state.settings.show_toolbar_auto, crate::app::i18n::tr(lng, "▶ Auto-read"));
                        ui.end_row();
                        ui.checkbox(&mut self.state.settings.show_toolbar_annotate, crate::app::i18n::tr(lng, "🖊 Annotate"));
                        ui.end_row();
                        ui.checkbox(&mut self.state.settings.show_toolbar_edit, crate::app::i18n::tr(lng, "✏ Page Edit"));
                        ui.end_row();
                    });
                    ui.checkbox(&mut self.state.settings.dark_mode, crate::app::i18n::tr(lng, "Dark Mode (Night)"));
                }
                1 => {
                    // ── Scrolling ──
                    ui.heading(crate::app::i18n::tr(lng, "Scrolling"));
                    ui.separator();
                    ui.label(crate::app::i18n::tr(lng, "Scroll Speed (px/s):"));
                    ui.add(egui::Slider::new(&mut self.state.settings.scroll_speed, 200.0..=4000.0)
                        .suffix(crate::app::i18n::tr(lng, " px/s")));
                    ui.label(crate::app::i18n::tr(lng, "摸鱼 Speed:"));
                    ui.add(egui::Slider::new(&mut self.state.settings.mo_yu_speed, 0.5..=5.0)
                        .suffix("x"));
                }
                2 => {
                    // ── Keyboard Shortcuts ──
                    ui.heading(crate::app::i18n::tr(lng, "Keyboard Shortcuts"));
                    ui.separator();
                    ui.label(crate::app::i18n::tr(lng, "Click a shortcut row to edit its key binding."));
                    ui.add_space(8.0);

                    egui::Grid::new("shortcuts_grid").num_columns(5).spacing([12.0, 4.0]).striped(true).show(ui, |ui| {
                        ui.strong(crate::app::i18n::tr(lng, "Action"));
                        ui.strong(crate::app::i18n::tr(lng, "Key"));
                        ui.strong("Ctrl");
                        ui.strong(crate::app::i18n::tr(lng, "Shift"));
                        ui.strong(crate::app::i18n::tr(lng, "Alt"));
                        ui.end_row();

                        let s = &mut self.state.settings;
                        for (i, action) in ALL_ACTIONS.iter().enumerate() {
                            let combo = s.shortcuts.get_mut(action);

                            ui.label(crate::app::i18n::tr(lng, action.label()));

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
                                    ui.label(crate::app::i18n::tr(lng, "(unset)"));
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
                                    ui.label(crate::app::i18n::tr(lng, "(unset)"));
                                }
                                ui.label(""); ui.label(""); ui.label("");
                                ui.label("");
                            }
                            ui.end_row();
                        }
                    });
                }
                3 => {
                    // ── Info ──
                    ui.heading("Info");
                    ui.separator();
                    ui.label(crate::app::i18n::tr(lng, "Config file: ./folix.conf"));
                    ui.horizontal(|ui| {
                        if ui.button(crate::app::i18n::tr(lng, "Reset Shortcuts to Default")).clicked() {
                            self.state.settings.shortcuts = crate::app::core::shortcuts::default_shortcuts();
                            self.state.settings.editing_shortcut = None;
                        }
                        if ui.button(crate::app::i18n::tr(lng, "Save Config Now")).clicked() {
                            self.save_config();
                            self.status_message = crate::app::i18n::tr(lng, "Config saved").to_string();
                        }
                    });
                }
                _ => {}
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

        // Settings tab
        if self.state.tabs[idx].is_settings_tab() {
            self.render_settings_tab(ui);
            return;
        }

        // PDF Toolbox tab
        if self.state.tabs[idx].is_pdf_toolbox() {
            let tab = &mut self.state.tabs[idx];
            if let Some(state) = tab.pdf_toolbox_mut() {
                pdf_toolbox::render_pdf_toolbox(ui, state);
            }
            return;
        }

        // Document tab
        let mode_name = self.state.tabs[idx].modes.active.name(&self.state.settings.language).to_string();
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
            &mut tab.modes.fit_mode,
            &mut tab.modes.view_rotation,
            &mut tab.modes.paginator,
            &mut tab.modes.reading,
            if is_light { Some(&mut tab.modes.auto) } else { None },
            if is_deep { Some(&mut tab.modes.annotate) } else { None },
            Some(ctx.clone()),
            dark_mode,
            &mut self.image_texture_cache,
        );

        // Handle pending vocabulary/sentence additions from context menu
        if let Some(word) = tab.modes.reading.selection.pending_vocab.take() {
            tab.modes.reading.vocab.push(Vocabulary {
                id: uuid::Uuid::new_v4().to_string(),
                word,
                context_sentence: None,
                definition: None,
                page: tab.modes.page,
            });
            tab.modes.reading.vocab_dirty = true;
            // Clear text selection after adding
            tab.modes.reading.selection.selected_word_indices.clear();
        }
        if let Some(text) = tab.modes.reading.selection.pending_sentence.take() {
            tab.modes.reading.sentences.push(Sentence_ {
                id: uuid::Uuid::new_v4().to_string(),
                text,
                page: tab.modes.page,
            });
            tab.modes.reading.sentences_dirty = true;
            tab.modes.reading.selection.selected_word_indices.clear();
        }

        // Sync annotations to database (only when dirty)
        if is_deep && tab.modes.annotate.dirty {
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
                    tab.modes.annotate.dirty = false;
                }
            }
        }

        // Sync vocabulary
        if tab.modes.reading.vocab_dirty {
            if let Some(ref db) = self.db {
                if let Some(book_id) = &tab.book_id {
                    let _ = db.delete_book_vocabulary(book_id);
                    for v in &tab.modes.reading.vocab {
                        let _ = db.add_vocabulary(
                            book_id, &v.word,
                            v.context_sentence.as_deref(),
                            v.definition.as_deref(),
                            v.page,
                        );
                    }
                    tab.modes.reading.vocab_dirty = false;
                }
            }
        }

        // Sync sentences
        if tab.modes.reading.sentences_dirty {
            if let Some(ref db) = self.db {
                if let Some(book_id) = &tab.book_id {
                    let _ = db.delete_book_sentences(book_id);
                    for s in &tab.modes.reading.sentences {
                        let _ = db.add_sentence(book_id, &s.text, s.page);
                    }
                    tab.modes.reading.sentences_dirty = false;
                }
            }
        }

        // Sync bookmarks
        if tab.modes.reading.bookmarks_dirty {
            if let Some(ref db) = self.db {
                if let Some(book_id) = &tab.book_id {
                    let _ = db.delete_book_bookmarks(book_id);
                    for bm in &tab.modes.reading.bookmarks {
                        let _ = db.add_bookmark(book_id, bm.page, Some(&bm.label));
                    }
                    tab.modes.reading.bookmarks_dirty = false;
                }
            }
        }

        // Render 摸鱼模式 viewport from current tab (light reading only)
        {
            let idx = self.state.active_tab;
            let is_light = self.state.tabs.get(idx)
                .map(|t| t.modes.active == ModeKind::LightReading)
                .unwrap_or(false);
            if is_light {
                let show = self.state.tabs[idx].modes.mo_yu.visible;
                if show {
                    let doc = self.state.tabs[idx].document.clone();
                    let mo_yu = &mut self.state.tabs[idx].modes.mo_yu;
                    mo_yu.speed = self.state.settings.mo_yu_speed;
                    ctx.show_viewport_immediate(
                        egui::ViewportId::from_hash_of("mo_yu_viewport"),
                        egui::ViewportBuilder::default()
                            .with_title("")
                            .with_inner_size(egui::vec2(400.0, 24.0))
                            .with_resizable(false)
                            .with_decorations(false),
                        |vp_ctx, class| {
                            // Position at bottom-right on first frame
                            if !mo_yu.positioned {
                                if let Some(monitor) = vp_ctx.input(|i| i.viewport().monitor_size) {
                                    let pos = egui::pos2(
                                        (monitor.x - 400.0 - 10.0).max(0.0),
                                        (monitor.y - 24.0 - 10.0).max(0.0),
                                    );
                                    vp_ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
                                }
                                mo_yu.positioned = true;
                            }
                            if class == egui::ViewportClass::Embedded {
                                let mut visible = mo_yu.visible;
                                let resp = egui::Window::new("")
                                    .id(egui::Id::new("mo_yu_window"))
                                    .open(&mut visible)
                                    .title_bar(false)
                                    .frame(egui::Frame::NONE)
                                    .default_size(egui::vec2(400.0, 24.0))
                                    .resizable(false)
                                    .show(vp_ctx, |ui| {
                                        mode_ui::render_mo_yu_ui(ui, mo_yu, &doc);
                                    });
                                if resp.is_some() {
                                    mo_yu.visible = visible;
                                } else {
                                    mo_yu.visible = false;
                                }
                            } else {
                                egui::CentralPanel::default()
                                    .frame(egui::Frame::NONE)
                                    .show(vp_ctx, |ui| {
                                        mode_ui::render_mo_yu_ui(ui, mo_yu, &doc);
                                    });
                                if vp_ctx.input(|i| i.viewport().close_requested()) {
                                    mo_yu.visible = false;
                                }
                            }
                        },
                    );
                }
            }
        }
    }

    fn render_toolbars(&mut self, ctx: &egui::Context) {
        let lng_s = self.state.settings.language.clone();
        let lng = &lng_s;
        let speed = self.state.settings.scroll_speed;
        let mut needs_reload: Option<String> = None;

        let show_nav = self.state.settings.show_toolbar_nav;
        let show_view = self.state.settings.show_toolbar_view;
        let show_page = self.state.settings.show_toolbar_page;

        // ── Row 1: mode tabs only ──
        egui::TopBottomPanel::bottom("toolbar_row1").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let tab = self.state.current_tab_mut();
                if tab.is_none() { return; }
                let tab = tab.unwrap();

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
                    if ui.selectable_label(selected, mk.name(lng)).clicked() {
                        tab.modes.switch_to(mk);
                    }
                }

                // ── Page number (right side) ──
                let doc_count = page_count_for_tab(tab);
                if doc_count > 0 && show_page {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("Pg {}/{}", tab.modes.page + 1, doc_count));
                    });
                }
            });
        });

        // ── Row 2: nav + view + page + mode-specific controls ──
        egui::TopBottomPanel::bottom("toolbar_row2").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let tab = self.state.current_tab_mut();
                if tab.is_none() { return; }
                let tab = tab.unwrap();

                let doc_count = page_count_for_tab(tab);
                let is_fixed_doc = tab.document.as_ref().map(|d| d.lock().is_fixed()).unwrap_or(true);

                if doc_count > 0 {
                    // ── Navigation section ──
                    if show_nav {
                        let is_paged = tab.modes.reading_layout == ReadingLayout::Paged;
                        if is_paged {
                            if ui.add_enabled(tab.modes.page > 0, egui::Button::new("◀")).clicked() {
                                page_jump(tab, tab.modes.page.saturating_sub(1));
                            }
                            if ui.add_enabled(tab.modes.page + 1 < doc_count, egui::Button::new("▶")).clicked() {
                                page_jump(tab, tab.modes.page + 1);
                            }
                        } else {
                            let up_btn = ui.add_enabled(
                                tab.modes.reading.scroll_offset_y > 0.0,
                                egui::Button::new("▲"),
                            );
                            if up_btn.clicked() || up_btn.is_pointer_button_down_on() {
                                tab.modes.reading.scroll_velocity = -speed;
                            }
                            let dn_btn = ui.button("▼");
                            if dn_btn.clicked() || dn_btn.is_pointer_button_down_on() {
                                tab.modes.reading.scroll_velocity = speed;
                            }
                        }
                        ui.separator();
                    }

                    // ── View adjustment section ──
                    if show_view {
                        let is_paged = tab.modes.reading_layout == ReadingLayout::Paged;
                        if is_fixed_doc {
                            let layout_label = if is_paged { crate::app::i18n::tr(lng, "Paged") } else { crate::app::i18n::tr(lng, "Scroll") };
                            if ui.button(layout_label).clicked() {
                                tab.modes.reading_layout = if is_paged { ReadingLayout::Scroll } else { ReadingLayout::Paged };
                            }
                        }

                        if is_fixed_doc {
                            let fit_w = tab.modes.fit_mode == FitMode::FitWidth;
                            if ui.selectable_label(fit_w, crate::app::i18n::tr(lng, "Fit Width")).clicked() {
                                tab.modes.fit_mode = if fit_w { FitMode::Free } else { FitMode::FitWidth };
                                tab.modes.scale = 1.0;
                            }
                            let fit_p = tab.modes.fit_mode == FitMode::FitPage;
                            if ui.selectable_label(fit_p, crate::app::i18n::tr(lng, "Fit Page")).clicked() {
                                tab.modes.fit_mode = if fit_p { FitMode::Free } else { FitMode::FitPage };
                                tab.modes.scale = 1.0;
                            }
                            if tab.modes.fit_mode != FitMode::Free {
                                if ui.button(crate::app::i18n::tr(lng, "Actual Size")).clicked() {
                                    tab.modes.fit_mode = FitMode::Free;
                                    tab.modes.scale = 1.0;
                                }
                            }
                        }

                        if is_fixed_doc {
                            if ui.button(crate::app::i18n::tr(lng, "↻ 90°")).clicked() {
                                let next = match tab.modes.view_rotation {
                                    ViewRotation::Deg0 => ViewRotation::Deg90,
                                    ViewRotation::Deg90 => ViewRotation::Deg180,
                                    ViewRotation::Deg180 => ViewRotation::Deg270,
                                    ViewRotation::Deg270 => ViewRotation::Deg0,
                                };
                                tab.modes.view_rotation = next;
                            }
                            if ui.button(crate::app::i18n::tr(lng, "↺ 90°")).clicked() {
                                let next = match tab.modes.view_rotation {
                                    ViewRotation::Deg0 => ViewRotation::Deg270,
                                    ViewRotation::Deg90 => ViewRotation::Deg0,
                                    ViewRotation::Deg180 => ViewRotation::Deg90,
                                    ViewRotation::Deg270 => ViewRotation::Deg180,
                                };
                                tab.modes.view_rotation = next;
                            }
                        }

                        ui.label("🔍");
                        let z = tab.modes.scale;
                        if ui.add_enabled(z > 0.1, egui::Button::new("−")).clicked() {
                            tab.modes.scale = (z - 0.1).max(0.1);
                            tab.modes.fit_mode = FitMode::Free;
                        }
                        let mut new_scale = tab.modes.scale;
                        let slider_range = 0.1..=10.0;
                        ui.add(egui::Slider::new(&mut new_scale, slider_range).text("×"));
                        if (new_scale - tab.modes.scale).abs() > 0.001 {
                            tab.modes.scale = new_scale;
                            tab.modes.fit_mode = FitMode::Free;
                        }
                        if ui.add_enabled(z < 10.0, egui::Button::new("+")).clicked() {
                            tab.modes.scale = (z + 0.1).min(10.0);
                            tab.modes.fit_mode = FitMode::Free;
                        }
                        ui.separator();
                    }
                }

                // ── Mode-specific controls ──
                match tab.modes.active {
                        ModeKind::LightReading => {
                            let play_label = if tab.modes.auto.playing { "⏸" } else { "▶" };
                            if ui.button(play_label).clicked() {
                                tab.modes.auto.playing = !tab.modes.auto.playing;
                                if tab.modes.auto.playing {
                                    tab.modes.auto.progress = 0.0;
                                }
                            }
                            ui.label(crate::app::i18n::tr(lng, "Speed:"));
                            ui.add(egui::Slider::new(&mut tab.modes.auto.speed, 0.5..=5.0).text("x"));

                            // 摸鱼模式 toggle (Light Reading only)
                            ui.separator();
                            let mo_yu_visible = tab.modes.mo_yu.visible;
                            let mo_yu_label = if mo_yu_visible { "🎵 摸鱼" } else { "摸鱼" };
                            if ui.selectable_label(mo_yu_visible, mo_yu_label).clicked() {
                                tab.modes.mo_yu.visible = !tab.modes.mo_yu.visible;
                                if tab.modes.mo_yu.visible {
                                    tab.modes.mo_yu.page = tab.modes.page;
                                    tab.modes.mo_yu.sentences.clear();
                                    tab.modes.mo_yu.playing = true;
                                    tab.modes.mo_yu.timer = 0.0;
                                    tab.modes.mo_yu.positioned = false;
                                }
                            }
                        }
                        ModeKind::DeepReading => {
                            let tool = &tab.modes.annotate.tool;
                            let is_sel = *tool == AnnotationTool::Highlight;
                            let is_pen = *tool == AnnotationTool::Pen;
                            let is_eraser = *tool == AnnotationTool::Eraser;
                            if ui.selectable_label(is_sel, crate::app::i18n::tr(lng, "Sel")).clicked() {
                                tab.modes.annotate.tool = AnnotationTool::Highlight;
                            }
                            if ui.selectable_label(is_pen, crate::app::i18n::tr(lng, "Pen")).clicked() {
                                tab.modes.annotate.tool = AnnotationTool::Pen;
                            }
                            if ui.selectable_label(is_eraser, crate::app::i18n::tr(lng, "Eraser")).clicked() {
                                tab.modes.annotate.tool = AnnotationTool::Eraser;
                            }
                            ui.separator();

                            // Highlight Selected button
                            let has_sel = !tab.modes.reading.selection.selected_word_indices.is_empty()
                                && tab.modes.reading.selection.page == tab.modes.page;
                            if ui.add_enabled(has_sel, egui::Button::new(crate::app::i18n::tr(lng, "High"))).clicked() {
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
                                        tab.modes.annotate.dirty = true;
                                        tab.modes.reading.selection.selected_word_indices.clear();
                                        tab.modes.reading.selection.anchor = None;
                                        tab.modes.reading.selection.focus = None;
                                    }
                                }
                            }

                            // Note on last highlight button
                            if ui.button(crate::app::i18n::tr(lng, "Note")).clicked() {
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
                            if ui.button(crate::app::i18n::tr(lng, "Undo")).clicked() {
                                tab.modes.annotate.annotations.pop();
                                tab.modes.annotate.dirty = true;
                            }
                            if ui.button(crate::app::i18n::tr(lng, "Clr")).clicked() {
                                tab.modes.annotate.annotations.clear();
                                tab.modes.annotate.dirty = true;
                            }
                        }
                        ModeKind::PageEdit => {
                            let path = tab.path.clone();
                            if let Some(ref p) = path {
                                if ui.button(crate::app::i18n::tr(lng, "↻ CW")).clicked() {
                                    let page = tab.modes.page;
                                    if edit_operations::rotate_page(p, page, 90).is_ok() {
                                        needs_reload = Some(p.clone());
                                    }
                                }
                                if ui.button(crate::app::i18n::tr(lng, "↻ CCW")).clicked() {
                                    let page = tab.modes.page;
                                    if edit_operations::rotate_page(p, page, 270).is_ok() {
                                        needs_reload = Some(p.clone());
                                    }
                                }
                                if ui.button(crate::app::i18n::tr(lng, "Del")).clicked() {
                                    let page = tab.modes.page;
                                    if page_count_for_tab(tab) > 1 {
                                        if edit_operations::delete_page(p, page).is_ok() {
                                            needs_reload = Some(p.clone());
                                        }
                                    }
                                }
                                if ui.button(crate::app::i18n::tr(lng, "+ Page")).clicked() {
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
                            if ui.button(crate::app::i18n::tr(lng, "A-")).clicked() {
                                state.font_size_scale = (state.font_size_scale - 0.1).max(0.5);
                            }
                            if ui.button(crate::app::i18n::tr(lng, "A+")).clicked() {
                                state.font_size_scale = (state.font_size_scale + 0.1).min(2.0);
                            }
                            ui.label(format!("{:.0}%", state.font_size_scale * 100.0));
                            if ui.selectable_label(state.bold, crate::app::i18n::tr(lng, "B")).clicked() {
                                state.bold = !state.bold;
                            }
                            if ui.selectable_label(state.italic, crate::app::i18n::tr(lng, "I")).clicked() {
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
                .add_filter("Documents", &["pdf", "epub", "txt", "md", "docx", "png", "jpg", "jpeg", "bmp", "gif", "webp", "tiff", "tif"])
                .pick_file();
            if let Some(path) = path {
                self.open_file(path.to_string_lossy().to_string());
            }
            self.open_dialog = false;
        }
    }

    fn render_about(&mut self, ctx: &egui::Context) {
        let lng_s = self.state.settings.language.clone();
        let lng = &lng_s;
        if self.show_about {
            egui::Window::new(crate::app::i18n::tr(lng, "About Folix"))
                .open(&mut self.show_about)
                .show(ctx, |ui| {
                    ui.heading(crate::app::i18n::tr(lng, "Folix"));
                    ui.label(format!("{} v0.1.0", crate::app::i18n::tr(lng, "PDF/EPUB Reader")));
                    ui.separator();
                    ui.label(crate::app::i18n::tr(lng, "Built with egui + mupdf"));
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
fn show_in_folder(path: &str) {
    let parent = std::path::Path::new(path).parent();
    let dir = parent.and_then(|p| p.to_str()).unwrap_or("");
    if dir.is_empty() { return; }
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg("/select,").arg(path).spawn();
}

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
