use std::path::PathBuf;
use std::sync::Arc;

use iced::widget::{button, column, container, image, row, scrollable, text};
use iced::{event, keyboard, window, Element, Length, Subscription, Task};
use parking_lot::Mutex as PmMutex;

use folix::app::engines::reflow_engine::ReflowDocument;

// ── PDF Support ──────────────────────────────────────────────

struct SafeDoc(PmMutex<mupdf::Document>);
unsafe impl Send for SafeDoc {}
unsafe impl Sync for SafeDoc {}

struct PdfDoc {
    doc: Arc<SafeDoc>,
    page_count: usize,
    current_page: usize,
    page_image: Option<image::Handle>,
    scale: f32,
}

// ── Reflow Support (EPUB/TXT) ────────────────────────────────

use folix::app::engines::{Document, ReflowLayout};
use parking_lot::Mutex as PlMutex;

struct ReflowDoc {
    doc: Arc<PlMutex<ReflowDocument>>,
    chapter_count: usize,
    current_chapter: usize,
    chapter_content: Vec<String>, // rendered text lines per block
}

// ── State & Messages ─────────────────────────────────────────

enum LoadedDoc {
    Pdf(PdfDoc),
    Reflow(ReflowDoc),
}

struct State {
    loaded: Option<LoadedDoc>,
    status: String,
}

fn main() -> iced::Result {
    env_logger::try_init().ok();
    iced::application(boot, update, view)
        .window(window::Settings {
            size: iced::Size::new(1200.0, 800.0),
            position: window::Position::Centered,
            ..window::Settings::default()
        })
        .subscription(subscription)
        .run()
}

fn boot() -> (State, Task<Message>) {
    (
        State {
            loaded: None,
            status: "Press Ctrl+O to open a file".into(),
        },
        Task::none(),
    )
}

#[derive(Debug, Clone)]
enum Message {
    OpenFile,
    FileSelected(Option<PathBuf>),
    PageRendered(Option<image::Handle>),
    NextPage,
    PrevPage,
    ZoomIn,
    ZoomOut,
    KeyPressed(keyboard::Key, keyboard::Modifiers),
}

// ── File Dialog ──────────────────────────────────────────────

fn open_file_dialog_task() -> Task<Message> {
    Task::perform(
        async {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("Documents", &["pdf", "epub", "txt", "md"])
                .pick_file()
                .await;
            file.map(|f| f.path().to_path_buf())
        },
        Message::FileSelected,
    )
}

// ── Update ───────────────────────────────────────────────────

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::OpenFile => open_file_dialog_task(),
        Message::FileSelected(Some(path)) => {
            state.status = format!("Loading: {}", path.display());
            let lower = path.to_string_lossy().to_lowercase();
            if lower.ends_with(".pdf") {
                match mupdf::Document::open(path.as_path()) {
                    Ok(doc) => {
                        let page_count = doc.page_count().unwrap_or(0) as usize;
                        let title = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Untitled");
                        let pd = PdfDoc {
                            doc: Arc::new(SafeDoc(PmMutex::new(doc))),
                            page_count,
                            current_page: 0,
                            page_image: None,
                            scale: 1.5,
                        };
                        state.loaded = Some(LoadedDoc::Pdf(pd));
                        state.status = format!("Loaded: {} ({} pages)", title, page_count);
                        return schedule_render(state);
                    }
                    Err(e) => {
                        state.status = format!("Failed to open PDF: {:?}", e);
                    }
                }
            } else if lower.ends_with(".epub") || lower.ends_with(".txt") {
                match ReflowDocument::open(path.to_str().unwrap_or("")) {
                    Some(doc) => {
                        let chapter_count = doc.chapter_count();
                        let title = doc.title();
                        let content = load_chapter_texts(&doc, 0);
                        let rd = ReflowDoc {
                            doc: Arc::new(PlMutex::new(doc)),
                            chapter_count,
                            current_chapter: 0,
                            chapter_content: content,
                        };
                        state.loaded = Some(LoadedDoc::Reflow(rd));
                        state.status = format!("Loaded: {} ({} chapters)", title, chapter_count);
                    }
                    None => {
                        state.status = "Failed to open document".into();
                    }
                }
            } else {
                state.status = format!("Unsupported file type: {}", path.display());
            }
            Task::none()
        }
        Message::FileSelected(None) => {
            state.status = "Open cancelled".into();
            Task::none()
        }
        Message::PageRendered(handle) => {
            if let Some(h) = handle {
                if let Some(LoadedDoc::Pdf(ref mut pdf)) = state.loaded {
                    pdf.page_image = Some(h);
                }
            } else {
                state.status = "Failed to render page".into();
            }
            Task::none()
        }
        Message::NextPage => {
            if let Some(LoadedDoc::Pdf(ref mut pdf)) = state.loaded {
                if pdf.current_page + 1 < pdf.page_count {
                    pdf.current_page += 1;
                    return schedule_render(state);
                }
            } else if let Some(LoadedDoc::Reflow(ref mut rd)) = state.loaded {
                if rd.current_chapter + 1 < rd.chapter_count {
                    rd.current_chapter += 1;
                    rd.chapter_content = load_chapter_texts(
                        &rd.doc.lock(),
                        rd.current_chapter,
                    );
                }
            }
            Task::none()
        }
        Message::PrevPage => {
            match &mut state.loaded {
                Some(LoadedDoc::Pdf(ref mut pdf)) => {
                    if pdf.current_page > 0 {
                        pdf.current_page -= 1;
                        return schedule_render(state);
                    }
                }
                Some(LoadedDoc::Reflow(ref mut rd)) => {
                    if rd.current_chapter > 0 {
                        rd.current_chapter -= 1;
                        rd.chapter_content = load_chapter_texts(
                            &rd.doc.lock(),
                            rd.current_chapter,
                        );
                    }
                }
                None => {}
            }
            Task::none()
        }
        Message::ZoomIn => {
            if let Some(LoadedDoc::Pdf(ref mut pdf)) = state.loaded {
                pdf.scale = (pdf.scale * 1.25).min(5.0);
                return schedule_render(state);
            }
            Task::none()
        }
        Message::ZoomOut => {
            if let Some(LoadedDoc::Pdf(ref mut pdf)) = state.loaded {
                pdf.scale = (pdf.scale / 1.25).max(0.2);
                return schedule_render(state);
            }
            Task::none()
        }
        Message::KeyPressed(key, modifiers) => {
            if modifiers.command() {
                if let keyboard::Key::Character(c) = key.as_ref() {
                    if c == "o" || c == "O" {
                        return open_file_dialog_task();
                    }
                }
            }
            match key.as_ref() {
                keyboard::Key::Named(keyboard::key::Named::ArrowRight)
                | keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                    let _ = update(state, Message::NextPage);
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowLeft)
                | keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                    let _ = update(state, Message::PrevPage);
                }
                keyboard::Key::Named(keyboard::key::Named::PageDown) => {
                    for _ in 0..5 {
                        let _ = update(state, Message::NextPage);
                    }
                }
                keyboard::Key::Named(keyboard::key::Named::PageUp) => {
                    for _ in 0..5 {
                        let _ = update(state, Message::PrevPage);
                    }
                }
                _ => {}
            }
            Task::none()
        }
    }
}

fn load_chapter_texts(doc: &ReflowDocument, idx: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let chapter = doc.chapter_text(idx);
    for line in chapter.lines() {
        lines.push(line.to_string());
    }
    if lines.is_empty() {
        lines.push("(empty chapter)".into());
    }
    lines
}

// ── Events ───────────────────────────────────────────────────

fn listen_to_events(
    event: event::Event,
    _status: event::Status,
    _id: window::Id,
) -> Option<Message> {
    match event {
        event::Event::Keyboard(keyboard::Event::KeyPressed {
            key, modifiers, ..
        }) => Some(Message::KeyPressed(key, modifiers)),
        _ => None,
    }
}

fn subscription(_state: &State) -> Subscription<Message> {
    event::listen_with(listen_to_events)
}

// ── Render ───────────────────────────────────────────────────

fn schedule_render(state: &mut State) -> Task<Message> {
    let (page, scale, doc_arc) = match state.loaded {
        Some(LoadedDoc::Pdf(ref pdf)) => {
            (pdf.current_page, pdf.scale, Arc::clone(&pdf.doc))
        }
        _ => return Task::none(),
    };

    Task::perform(
        async move {
            let guard = doc_arc.0.lock();
            let page_obj = guard.load_page(page as i32).ok()?;
            let cs = mupdf::Colorspace::device_rgb();
            let ctm = mupdf::Matrix::new_scale(scale, scale);
            let pixmap = page_obj.to_pixmap(&ctm, &cs, true, true).ok()?;
            let w = pixmap.width();
            let h = pixmap.height();
            let samples = pixmap.samples().to_vec();
            let n = pixmap.n() as usize;
            drop(guard);

            let rgba = if n == 4 {
                samples
            } else {
                let mut data = Vec::with_capacity(samples.len() / 3 * 4);
                for chunk in samples.chunks(3) {
                    data.extend_from_slice(&chunk[..3]);
                    data.push(255);
                }
                data
            };

            Some(image::Handle::from_rgba(w, h, rgba))
        },
        Message::PageRendered,
    )
}

// ── View ─────────────────────────────────────────────────────

fn view(state: &State) -> Element<'_, Message> {
    let content: Element<'_, Message> = match state.loaded {
        Some(LoadedDoc::Pdf(ref pdf)) => view_pdf(pdf),
        Some(LoadedDoc::Reflow(ref rd)) => view_reflow(rd),
        None => column![
            text("Folix - Iced Reader").size(24),
            text(&state.status).size(14),
            button("Open File...").on_press(Message::OpenFile),
        ]
        .spacing(8)
        .padding(16)
        .into(),
    };

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_pdf(doc: &PdfDoc) -> Element<'_, Message> {
    let page_info = text(format!(
        "Page {}/{}  Scale: {:.0}%",
        doc.current_page + 1,
        doc.page_count,
        doc.scale * 100.0,
    ))
    .size(14);

    let nav = row![
        button("◀").on_press(Message::PrevPage),
        text(format!(" {} ", doc.current_page + 1)).size(14),
        button("▶").on_press(Message::NextPage),
        button("-").on_press(Message::ZoomOut),
        button("+").on_press(Message::ZoomIn),
    ]
    .spacing(4);

    let img: Element<'_, Message> = if let Some(ref handle) = doc.page_image {
        image(handle.clone())
            .width(Length::Fill)
            .height(Length::Shrink)
            .into()
    } else {
        text("Rendering...").into()
    };

    let scroll = scrollable(img)
        .width(Length::Fill)
        .height(Length::Fill);

    column![page_info, nav, scroll,].spacing(8).padding(8).into()
}

fn view_reflow(doc: &ReflowDoc) -> Element<'_, Message> {
    let header = text(format!(
        "Chapter {}/{}",
        doc.current_chapter + 1,
        doc.chapter_count,
    ))
    .size(14);

    let nav = row![
        button("◀").on_press(Message::PrevPage),
        text(format!(" {} ", doc.current_chapter + 1)).size(14),
        button("▶").on_press(Message::NextPage),
    ]
    .spacing(4);

    let body: Element<'_, Message> = {
        let mut col = column![].spacing(2).padding(16);
        for line in &doc.chapter_content {
            col = col.push(text(line).size(16));
        }
        col.into()
    };

    let scroll = scrollable(body)
        .width(Length::Fill)
        .height(Length::Fill);

    column![header, nav, scroll,].spacing(8).padding(8).into()
}
