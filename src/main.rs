mod app;
mod geometry;
mod maze;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 820.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Mazui",
        options,
        Box::new(|cc| Ok(Box::new(app::MazeApp::new(cc)))),
    )
}
