use std::cell::RefCell;
use std::rc::Rc;
use slint::ComponentHandle;
use folix::slint_app::MainWindow;
use folix::slint_app::reflow_viewer::ReflowViewerState;
use folix::slint_app::pdf_viewer::PdfViewerState;

fn update_reflow_ui(window: &MainWindow, state: &ReflowViewerState) {
    window.set_show_reflow(true);
    window.set_show_pdf(false);
    window.set_reflow_content(state.current_text().into());
    window.set_chapter_title(state.current_title().into());
    window.set_current_chapter(state.current_chapter_index() as i32);
    window.set_total_chapters(state.chapter_count() as i32);
}

fn update_pdf_ui(window: &MainWindow, state: &PdfViewerState) {
    window.set_show_pdf(true);
    window.set_show_reflow(false);
    window.set_page_title(state.document_title().into());
    window.set_current_page(state.current_page_index() as i32);
    window.set_total_pages(state.page_count() as i32);
    if let Some(image) = state.render_current_page() {
        window.set_page_image(image);
    }
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
    let reflow_state = Rc::new(RefCell::new(ReflowViewerState::new()));
    let pdf_state = Rc::new(RefCell::new(PdfViewerState::new()));

    // Open file
    {
        let window_weak = window.as_weak();
        let reflow_state = reflow_state.clone();
        let pdf_state = pdf_state.clone();
        window.on_open_file(move || {
            let window = window_weak.unwrap();
            let file = rfd::FileDialog::new()
                .add_filter("Documents", &["pdf", "epub", "txt", "md", "docx"])
                .pick_file();

            let Some(path) = file else { return };
            let path_str = path.to_string_lossy().to_string();
            let is_pdf = path_str.to_lowercase().ends_with(".pdf");

            if is_pdf {
                let mut pdf = pdf_state.borrow_mut();
                match pdf.open_file(&path_str) {
                    Ok(_) => {
                        drop(pdf);
                        update_pdf_ui(&window, &pdf_state.borrow());
                        window.set_status_text(
                            format!("Opened PDF: {}", path.file_name().unwrap_or_default().to_string_lossy()).into()
                        );
                    }
                    Err(e) => {
                        window.set_status_text(format!("Error: {}", e).into());
                    }
                }
            } else {
                let mut reflow = reflow_state.borrow_mut();
                match reflow.open_file(&path_str) {
                    Ok(_) => {
                        drop(reflow);
                        update_reflow_ui(&window, &reflow_state.borrow());
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

    // Prev chapter
    {
        let window_weak = window.as_weak();
        let state = reflow_state.clone();
        window.on_prev_chapter(move || {
            let window = window_weak.unwrap();
            state.borrow_mut().prev_chapter();
            update_reflow_ui(&window, &state.borrow());
        });
    }

    // Next chapter
    {
        let window_weak = window.as_weak();
        let state = reflow_state.clone();
        window.on_next_chapter(move || {
            let window = window_weak.unwrap();
            state.borrow_mut().next_chapter();
            update_reflow_ui(&window, &state.borrow());
        });
    }

    // Prev page
    {
        let window_weak = window.as_weak();
        let state = pdf_state.clone();
        window.on_prev_page(move || {
            let window = window_weak.unwrap();
            state.borrow_mut().prev_page();
            update_pdf_ui(&window, &state.borrow());
        });
    }

    // Next page
    {
        let window_weak = window.as_weak();
        let state = pdf_state.clone();
        window.on_next_page(move || {
            let window = window_weak.unwrap();
            state.borrow_mut().next_page();
            update_pdf_ui(&window, &state.borrow());
        });
    }

    // PDF page click
    {
        let window_weak = window.as_weak();
        let state = pdf_state.clone();
        window.on_page_clicked(move |x, y| {
            let window = window_weak.unwrap();
            state.borrow_mut().handle_click(x, y);
            update_pdf_ui(&window, &state.borrow());

            let text = state.borrow().selected_text();
            if !text.is_empty() {
                window.set_status_text(format!("Selected: \"{}\"", &text[..text.len().min(50)]).into());
            } else {
                window.set_status_text("Selection cleared".into());
            }
        });
    }

    // PDF copy selected
    {
        let window_weak = window.as_weak();
        let state = pdf_state.clone();
        window.on_copy_selected(move || {
            let window = window_weak.unwrap();
            let text = state.borrow().selected_text();
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
