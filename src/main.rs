use folix::app::ui::shell::FolixApp;

#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

fn main() -> eframe::Result<()> {
    let _ = env_logger::try_init();

    // Load language before creating the window
    let lang = folix::app::config::ConfigData::load()
        .map(|c| c.settings.language)
        .unwrap_or_else(|| "zh-CN".into());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 850.0])
            .with_title(folix::app::i18n::tr(&lang, "Folix")),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Folix",
        options,
        Box::new(|cc| Ok(Box::new(FolixApp::new(cc)))),
    )
}
