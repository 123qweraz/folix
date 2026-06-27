use folix::app::ui::shell::FolixApp;

fn main() -> eframe::Result<()> {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("Folix"),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Folix",
        options,
        Box::new(|cc| Ok(Box::new(FolixApp::new(cc)))),
    )
}
