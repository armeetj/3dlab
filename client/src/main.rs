mod app;

use app::App;

fn main() -> eframe::Result<()> {
    env_logger::init();
    log::info!("Starting 3DLab client (native)...");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("3DLab - MRI Volume Viewer"),
        ..Default::default()
    };

    eframe::run_native(
        "3DLab",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
