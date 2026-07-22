use iced::widget::{column, text};
use iced::{event, keyboard, window, Element, Subscription, Task};

use folix::iced_app::state::{
    self, boot, DocumentHolder, Message, TabContent,
};
use folix::iced_app::{tab_bar, home_page, settings, pdf_toolbox, pdf_viewer, reflow_viewer};

use folix::iced_app::state::State;

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

// ── Update ───────────────────────────────────────────────────

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::OpenFile => open_file_dialog_task(),

        Message::FileSelected(Some(path)) => {
            if let Some((path, doc)) = state::load_document(path) {
                let title = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Document")
                    .to_string();
                let tabs = &mut state.tabs;
                // Check if current tab is Home, replace it
                let is_home = matches!(tabs[state.active_tab].content, TabContent::Home);
                if is_home {
                    tabs[state.active_tab] = state::Tab {
                        title: title.clone(),
                        content: TabContent::Document {
                            path: path.clone(),
                            document: doc,
                            current_page: 0,
                            scale: 1.5,
                            page_image: None,
                        },
                        path: Some(path.clone()),
                    };
                } else {
                    tabs.push(state::Tab {
                        title: title.clone(),
                        content: TabContent::Document {
                            path: path.clone(),
                            document: doc,
                            current_page: 0,
                            scale: 1.5,
                            page_image: None,
                        },
                        path: Some(path.clone()),
                    });
                    state.active_tab = tabs.len() - 1;
                }
                state.status = format!("Loaded: {}", title);
                return schedule_render(state);
            }
            state.status = "Failed to open document".into();
            Task::none()
        }

        Message::FileSelected(None) => {
            state.status = "Open cancelled".into();
            Task::none()
        }

        Message::PageRendered(tab_idx, handle) => {
            if tab_idx < state.tabs.len() {
                if let TabContent::Document {
                    ref mut page_image, ..
                } = state.tabs[tab_idx].content
                {
                    *page_image = handle;
                }
            }
            Task::none()
        }

        Message::CloseTab(idx) => {
            if state.tabs.len() > 1 {
                state.tabs.remove(idx);
                if state.active_tab >= state.tabs.len() {
                    state.active_tab = state.tabs.len() - 1;
                }
            }
            Task::none()
        }

        Message::ActivateTab(idx) => {
            state.active_tab = idx;
            Task::none()
        }

        Message::AddHomeTab => {
            state.tabs.push(state::Tab {
                title: "Home".into(),
                content: TabContent::Home,
                path: None,
            });
            state.active_tab = state.tabs.len() - 1;
            Task::none()
        }

        Message::AddSettingsTab => {
            // Settings is singleton
            let exists = state
                .tabs
                .iter()
                .any(|t| matches!(t.content, TabContent::Settings));
            if !exists {
                state.tabs.push(state::Tab {
                    title: "Settings".into(),
                    content: TabContent::Settings,
                    path: None,
                });
                state.active_tab = state.tabs.len() - 1;
            }
            Task::none()
        }

        Message::AddPdfToolboxTab => {
            let exists = state
                .tabs
                .iter()
                .any(|t| matches!(t.content, TabContent::PdfToolbox));
            if !exists {
                state.tabs.push(state::Tab {
                    title: "PDF Tools".into(),
                    content: TabContent::PdfToolbox,
                    path: None,
                });
                state.active_tab = state.tabs.len() - 1;
            }
            Task::none()
        }

        Message::NextPage => {
            let tab_idx = state.active_tab;
            if let TabContent::Document {
                ref mut current_page,
                document:
                    DocumentHolder::Pdf(ref holder),
                ..
            } = state.tabs[tab_idx].content
            {
                if *current_page + 1 < holder.page_count {
                    *current_page += 1;
                    return schedule_render(state);
                }
            } else if let TabContent::Document {
                document:
                    DocumentHolder::Reflow(ref mut holder),
                ..
            } = state.tabs[tab_idx].content
            {
                if holder.current_chapter + 1 < holder.chapter_count {
                    holder.current_chapter += 1;
                    holder.chapter_lines =
                        state::load_chapter_texts_locked(&holder.doc, holder.current_chapter);
                }
            }
            Task::none()
        }

        Message::PrevPage => {
            let tab_idx = state.active_tab;
            if let TabContent::Document {
                ref mut current_page,
                document:
                    DocumentHolder::Pdf(..),
                ..
            } = state.tabs[tab_idx].content
            {
                if *current_page > 0 {
                    *current_page -= 1;
                    return schedule_render(state);
                }
            } else if let TabContent::Document {
                document:
                    DocumentHolder::Reflow(ref mut holder),
                ..
            } = state.tabs[tab_idx].content
            {
                if holder.current_chapter > 0 {
                    holder.current_chapter -= 1;
                    holder.chapter_lines =
                        state::load_chapter_texts_locked(&holder.doc, holder.current_chapter);
                }
            }
            Task::none()
        }

        Message::ZoomIn => {
            let tab_idx = state.active_tab;
            if let TabContent::Document {
                ref mut scale, ..
            } = state.tabs[tab_idx].content
            {
                *scale = (*scale * 1.25).min(5.0);
                return schedule_render(state);
            }
            Task::none()
        }

        Message::ZoomOut => {
            let tab_idx = state.active_tab;
            if let TabContent::Document {
                ref mut scale, ..
            } = state.tabs[tab_idx].content
            {
                *scale = (*scale / 1.25).max(0.2);
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
                    if c == "w" || c == "W" {
                        if state.tabs.len() > 1 {
                            state.tabs.remove(state.active_tab);
                            if state.active_tab >= state.tabs.len() {
                                state.active_tab = state.tabs.len() - 1;
                            }
                        }
                    }
                    if c == "t" || c == "T" {
                        return update(state, Message::AddHomeTab);
                    }
                }
            }
            match key.as_ref() {
                keyboard::Key::Named(keyboard::key::Named::ArrowRight)
                | keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                    return update(state, Message::NextPage);
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowLeft)
                | keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                    return update(state, Message::PrevPage);
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

        Message::NavLeft | Message::NavRight | Message::NavUp | Message::NavDown => {
            Task::none()
        }

        Message::SettingsChanged(settings) => {
            state.settings = settings;
            Task::none()
        }

        Message::PdfOperation(_) => Task::none(),
    }
}

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

fn schedule_render(state: &mut State) -> Task<Message> {
    let tab_idx = state.active_tab;
    match &state.tabs[tab_idx].content {
        TabContent::Document {
            document: DocumentHolder::Pdf(holder),
            current_page,
            scale,
            ..
        } => {
            let doc_arc = holder.doc.clone();
            let page = *current_page;
            let scale_f = *scale;
            Task::perform(
                async move {
                    let guard = doc_arc.0.lock();
                    let page_obj = guard.load_page(page as i32).ok()?;
                    let cs = mupdf::Colorspace::device_rgb();
                    let ctm = mupdf::Matrix::new_scale(scale_f, scale_f);
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
                    Some(iced::widget::image::Handle::from_rgba(w, h, rgba))
                },
                move |handle| Message::PageRendered(tab_idx, handle),
            )
        }
        _ => Task::none(),
    }
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

// ── View ─────────────────────────────────────────────────────

fn view(state: &State) -> Element<'_, Message> {
    let tab_bar = tab_bar::view(state);

    let content: Element<'_, Message> = if state.tabs.is_empty() {
        text("No tabs open").into()
    } else {
        let tab = &state.tabs[state.active_tab];
        match &tab.content {
            TabContent::Home => home_page::view(state),
            TabContent::Settings => settings::view(state),
            TabContent::PdfToolbox => pdf_toolbox::view(state),
            TabContent::Document { .. } => {
                // Check if PDF or Reflow
                match &tab.content {
                    TabContent::Document {
                        document: DocumentHolder::Pdf(_),
                        ..
                    } => pdf_viewer::view(tab),
                    TabContent::Document {
                        document: DocumentHolder::Reflow(_),
                        ..
                    } => reflow_viewer::view(tab),
                    _ => text("Unknown document type").into(),
                }
            }
        }
    };

    column![tab_bar, content,].spacing(0).into()
}
