mod app;
mod renderer;

pub use app::App;
pub use renderer::{Camera, VolumeRenderer};

// WASM entry point
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    // Set up panic hook for better error messages
    console_error_panic_hook::set_once();

    // Set up logging
    console_log::init_with_level(log::Level::Debug).expect("Failed to init logger");

    log::info!("Starting 3DLab client...");

    // Create canvas element
    let window = web_sys::window().expect("No window");
    let document = window.document().expect("No document");

    let canvas = document
        .create_element("canvas")
        .expect("Failed to create canvas")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("Not a canvas");

    canvas.set_id("the_canvas_id");

    // Set canvas to fill the window
    let width = window.inner_width().unwrap().as_f64().unwrap() as u32;
    let height = window.inner_height().unwrap().as_f64().unwrap() as u32;
    canvas.set_width(width);
    canvas.set_height(height);

    // Style canvas to fill viewport
    let style = canvas.style();
    style.set_property("width", "100%").unwrap();
    style.set_property("height", "100%").unwrap();
    style.set_property("position", "absolute").unwrap();
    style.set_property("top", "0").unwrap();
    style.set_property("left", "0").unwrap();

    document
        .body()
        .expect("No body")
        .append_child(&canvas)
        .expect("Failed to append canvas");

    // Start the app with glow (OpenGL/WebGL2)
    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async move {
        log::info!("Starting eframe...");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| {
                    log::info!("Creating app, glow available: {}", cc.gl.is_some());
                    Ok(Box::new(App::new(cc)))
                }),
            )
            .await;

        // Hide loading message after start
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(loading) = document.get_element_by_id("loading") {
                    let _ = loading.set_attribute("style", "display: none");
                }
            }
        }

        match &start_result {
            Ok(_) => log::info!("eframe started successfully"),
            Err(e) => log::error!("Failed to start eframe: {:?}", e),
        }
    });

    Ok(())
}
