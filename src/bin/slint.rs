use std::cell::RefCell;
use std::rc::Rc;
use slint::ComponentHandle;
use folix::slint_app::MainWindow;
use folix::slint_app::reflow_viewer::ReflowViewerState;

fn update_ui(window: &MainWindow, state: &ReflowViewerState) {
    window.set_show_reflow(true);
    window.set_reflow_content(state.current_text().into());
    window.set_chapter_title(state.current_title().into());
    window.set_current_chapter(state.current_chapter_index() as i32);
    window.set_total_chapters(state.chapter_count() as i32);
}

fn main() {
    let window = MainWindow::new().unwrap();
    let state = Rc::new(RefCell::new(ReflowViewerState::new()));

    {
        let window_weak = window.as_weak();
        let state = state.clone();
        window.on_open_file(move || {
            let window = window_weak.unwrap();
            let mut state = state.borrow_mut();

            let file = rfd::FileDialog::new()
                .add_filter("Documents", &["pdf", "epub", "txt", "md", "docx"])
                .pick_file();

            if let Some(path) = file {
                let path_str = path.to_string_lossy().to_string();
                match state.open_file(&path_str) {
                    Ok(_) => {
                        update_ui(&window, &state);
                        window.set_status_text(
                            format!("Opened: {}", path.file_name().unwrap_or_default().to_string_lossy()).into()
                        );
                    }
                    Err(e) => {
                        window.set_status_text(format!("Error: {}", e).into());
                    }
                }
            }
        });
    }

    {
        let window_weak = window.as_weak();
        let state = state.clone();
        window.on_prev_chapter(move || {
            let window = window_weak.unwrap();
            let mut state = state.borrow_mut();
            state.prev_chapter();
            update_ui(&window, &state);
        });
    }

    {
        let window_weak = window.as_weak();
        let state = state.clone();
        window.on_next_chapter(move || {
            let window = window_weak.unwrap();
            let mut state = state.borrow_mut();
            state.next_chapter();
            update_ui(&window, &state);
        });
    }

    window.run().unwrap();
}
