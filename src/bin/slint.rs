use std::cell::RefCell;
use std::rc::Rc;
use slint::ComponentHandle;
use folix::slint_app::{MainWindow, TabInfo, RecentFileInfo};
use folix::slint_app::state::AppState;
use folix::app::config::{ConfigData, RecentFile};

fn update_tab_model(window: &MainWindow, state: &AppState) {
    let model = slint::VecModel::<TabInfo>::default();
    for (i, tab) in state.tabs.iter().enumerate() {
        model.push(TabInfo {
            title: tab.title_for_display().into(),
            is_active: i == state.active_tab,
        });
    }
    window.set_tabs(slint::ModelRc::from(std::rc::Rc::new(model)));
}

fn update_active_tab(window: &MainWindow, state: &AppState) {
    let Some(tab) = state.active_tab() else {
        window.set_show_home(true);
        window.set_show_reflow(false);
        window.set_show_pdf(false);
        return;
    };

    window.set_show_home(false);

        match &tab.content {
            folix::slint_app::state::TabContent::Home => {
                window.set_show_home(true);
                window.set_show_reflow(false);
                window.set_show_pdf(false);
            }
            folix::slint_app::state::TabContent::Reflow(rstate) => {
            window.set_show_reflow(true);
            window.set_show_pdf(false);
            window.set_reflow_content(rstate.current_text().into());
            window.set_chapter_title(rstate.current_title().into());
            window.set_current_chapter(rstate.current_chapter_index() as i32);
            window.set_total_chapters(rstate.chapter_count() as i32);
        }
        folix::slint_app::state::TabContent::Pdf(pstate) => {
            window.set_show_pdf(true);
            window.set_show_reflow(false);
            window.set_page_title(pstate.document_title().into());
            window.set_current_page(pstate.current_page_index() as i32);
            window.set_total_pages(pstate.page_count() as i32);
            if let Some(image) = pstate.render_current_page() {
                window.set_page_image(image);
            }
        }
    }
}

fn refresh_ui(window: &MainWindow, state: &AppState) {
    update_tab_model(window, state);
    update_active_tab(window, state);
}

fn update_recent_files(window: &MainWindow, files: &[RecentFile]) {
    let model = slint::VecModel::<RecentFileInfo>::default();
    for file in files {
        let title = std::path::Path::new(&file.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        model.push(RecentFileInfo {
            path: file.path.clone().into(),
            title: title.into(),
        });
    }
    window.set_recent_files(slint::ModelRc::from(std::rc::Rc::new(model)));
}

fn copy_to_clipboard(text: &str) {
    use copypasta::ClipboardContext;
    use copypasta::ClipboardProvider;
    if let Ok(mut ctx) = ClipboardContext::new() {
        let _ = ctx.set_contents(text.to_string());
    }
}

fn main() {
    let window = MainWindow::new().unwrap();
    let state = Rc::new(RefCell::new(AppState::new()));

    // Load recent files
    let config = ConfigData::load().unwrap_or_else(|| ConfigData {
        settings: Default::default(),
        recent_files: vec![],
    });
    update_recent_files(&window, &config.recent_files);

    // Open file (button / home page card)
    {
        let w = window.as_weak();
        let s = state.clone();
        window.on_open_file(move || {
            let window = w.unwrap();
            let file = rfd::FileDialog::new()
                .add_filter("Documents", &["pdf", "epub", "txt", "md", "docx"])
                .pick_file();
            let Some(path) = file else { return };
            let path_str = path.to_string_lossy().to_string();

            let mut guard = s.borrow_mut();
            if guard.open_file(&path_str).is_ok() {
                drop(guard);
                refresh_ui(&window, &s.borrow());
                window.set_status_text(
                    format!("Opened: {}", path.file_name().unwrap_or_default().to_string_lossy()).into()
                );
            } else {
                window.set_status_text("Failed to open file".into());
            }
        });
    }

    // Open recent file
    {
        let w = window.as_weak();
        let s = state.clone();
        window.on_open_recent(move |path| {
            let window = w.unwrap();
            let mut guard = s.borrow_mut();
            if guard.open_file(path.as_str()).is_ok() {
                drop(guard);
                refresh_ui(&window, &s.borrow());
                window.set_status_text(format!("Opened recent").into());
            } else {
                window.set_status_text("Failed to open file".into());
            }
        });
    }

    // New home tab
    {
        let w = window.as_weak();
        let s = state.clone();
        window.on_new_home_tab(move || {
            let window = w.unwrap();
            s.borrow_mut().new_home_tab();
            refresh_ui(&window, &s.borrow());
            window.set_status_text("New tab".into());
        });
    }

    // Open settings
    {
        let w = window.as_weak();
        window.on_open_settings(move || {
            let window = w.unwrap();
            window.set_status_text("Settings - coming soon".into());
        });
    }

    // Open PDF toolbox
    {
        let w = window.as_weak();
        window.on_open_pdf_toolbox(move || {
            let window = w.unwrap();
            window.set_status_text("PDF Toolbox - coming soon".into());
        });
    }

    // Switch tab
    {
        let w = window.as_weak();
        let s = state.clone();
        window.on_switch_tab(move |idx| {
            let window = w.unwrap();
            let idx = if idx >= 0 { idx as usize } else { return };
            s.borrow_mut().switch_to_tab(idx);
            refresh_ui(&window, &s.borrow());
        });
    }

    // Close tab
    {
        let w = window.as_weak();
        let s = state.clone();
        window.on_close_tab(move |idx| {
            let window = w.unwrap();
            let idx = if idx >= 0 { idx as usize } else { s.borrow().active_tab };
            s.borrow_mut().close_tab(idx);
            refresh_ui(&window, &s.borrow());
        });
    }

    // Prev chapter
    {
        let w = window.as_weak();
        let s = state.clone();
        window.on_prev_chapter(move || {
            let window = w.unwrap();
            let mut guard = s.borrow_mut();
            if let Some(tab) = guard.active_tab_mut() {
                if let folix::slint_app::state::TabContent::Reflow(r) = &mut tab.content {
                    r.prev_chapter();
                }
            }
            drop(guard);
            refresh_ui(&window, &s.borrow());
        });
    }

    // Next chapter
    {
        let w = window.as_weak();
        let s = state.clone();
        window.on_next_chapter(move || {
            let window = w.unwrap();
            let mut guard = s.borrow_mut();
            if let Some(tab) = guard.active_tab_mut() {
                if let folix::slint_app::state::TabContent::Reflow(r) = &mut tab.content {
                    r.next_chapter();
                }
            }
            drop(guard);
            refresh_ui(&window, &s.borrow());
        });
    }

    // Prev page
    {
        let w = window.as_weak();
        let s = state.clone();
        window.on_prev_page(move || {
            let window = w.unwrap();
            let mut guard = s.borrow_mut();
            if let Some(tab) = guard.active_tab_mut() {
                if let folix::slint_app::state::TabContent::Pdf(p) = &mut tab.content {
                    p.prev_page();
                }
            }
            drop(guard);
            refresh_ui(&window, &s.borrow());
        });
    }

    // Next page
    {
        let w = window.as_weak();
        let s = state.clone();
        window.on_next_page(move || {
            let window = w.unwrap();
            let mut guard = s.borrow_mut();
            if let Some(tab) = guard.active_tab_mut() {
                if let folix::slint_app::state::TabContent::Pdf(p) = &mut tab.content {
                    p.next_page();
                }
            }
            drop(guard);
            refresh_ui(&window, &s.borrow());
        });
    }

    // PDF page click
    {
        let w = window.as_weak();
        let s = state.clone();
        window.on_page_clicked(move |x, y| {
            let window = w.unwrap();
            let mut guard = s.borrow_mut();
            if let Some(tab) = guard.active_tab_mut() {
                if let folix::slint_app::state::TabContent::Pdf(p) = &mut tab.content {
                    p.handle_click(x, y);
                }
            }
            drop(guard);

            let guard = s.borrow();
            let text = guard.active_tab()
                .and_then(|t| {
                    if let folix::slint_app::state::TabContent::Pdf(p) = &t.content {
                        Some(p.selected_text())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();

            if !text.is_empty() {
                window.set_status_text(format!("Selected: \"{}\"", &text[..text.len().min(50)]).into());
            } else {
                window.set_status_text("Selection cleared".into());
            }
            refresh_ui(&window, &guard);
        });
    }

    // PDF copy
    {
        let w = window.as_weak();
        let s = state.clone();
        window.on_copy_selected(move || {
            let window = w.unwrap();
            let guard = s.borrow();
            let text = guard.active_tab()
                .and_then(|t| {
                    if let folix::slint_app::state::TabContent::Pdf(p) = &t.content {
                        Some(p.selected_text())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();

            if !text.is_empty() {
                copy_to_clipboard(&text);
                window.set_status_text(format!("Copied: \"{}\"", &text[..text.len().min(50)]).into());
            } else {
                window.set_status_text("Nothing to copy".into());
            }
        });
    }

    window.run().unwrap();
}
