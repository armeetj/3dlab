use eframe::egui;
use shared::{VolumeInfo, VolumeListResponse};
use std::sync::{Arc, Mutex};

/// Shared state for async operations
#[derive(Default)]
struct AsyncState {
    volumes: Option<Result<Vec<VolumeInfo>, String>>,
}

/// Main application state
pub struct App {
    /// List of available volumes from server
    volumes: Vec<VolumeInfo>,
    /// Currently selected volume ID
    selected_volume: Option<String>,
    /// Loading state
    loading: bool,
    /// Error message if any
    error: Option<String>,
    /// Server base URL
    api_base: String,
    /// Shared state for async callbacks
    async_state: Arc<Mutex<AsyncState>>,
}

impl App {
    /// Create dark mode visuals with flat style
    fn dark_visuals() -> egui::Visuals {
        let mut visuals = egui::Visuals::dark();

        // Remove all rounding
        visuals.window_rounding = egui::Rounding::ZERO;
        visuals.menu_rounding = egui::Rounding::ZERO;
        visuals.widgets.noninteractive.rounding = egui::Rounding::ZERO;
        visuals.widgets.inactive.rounding = egui::Rounding::ZERO;
        visuals.widgets.hovered.rounding = egui::Rounding::ZERO;
        visuals.widgets.active.rounding = egui::Rounding::ZERO;
        visuals.widgets.open.rounding = egui::Rounding::ZERO;

        // Remove expansion on hover (keeps button same size)
        visuals.widgets.hovered.expansion = 0.0;
        visuals.widgets.active.expansion = 0.0;

        // Color-based hover/click instead of outlines
        // Inactive: dark gray
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(45, 45, 45);
        visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(180, 180, 180));

        // Hovered: lighter gray
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(70, 70, 70);
        visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 220, 220));

        // Active/clicked: even lighter
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(90, 90, 90);
        visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);

        // Open (dropdown open, etc)
        visuals.widgets.open.bg_fill = egui::Color32::from_rgb(60, 60, 60);
        visuals.widgets.open.bg_stroke = egui::Stroke::NONE;

        // Panel backgrounds
        visuals.panel_fill = egui::Color32::from_rgb(25, 25, 25);
        visuals.window_fill = egui::Color32::from_rgb(30, 30, 30);

        // Selection color
        visuals.selection.bg_fill = egui::Color32::from_rgb(60, 80, 120);

        // Popup shadow
        visuals.popup_shadow = egui::epaint::Shadow::NONE;

        visuals
    }

    /// Create style with reduced padding
    fn flat_style() -> egui::Style {
        let mut style = egui::Style::default();
        style.visuals = Self::dark_visuals();

        // Reduce padding in buttons/widgets
        style.spacing.button_padding = egui::vec2(4.0, 2.0);
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);

        // Reduce combo box padding
        style.spacing.combo_width = 0.0;

        // Menu/popup padding
        style.spacing.menu_margin = egui::Margin::same(2.0);
        style.spacing.window_margin = egui::Margin::same(4.0);

        style
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Apply flat style on startup
        cc.egui_ctx.set_style(Self::flat_style());

        // Scale UI larger (1.5x default size)
        cc.egui_ctx.set_pixels_per_point(1.5);
        let api_base = if cfg!(target_arch = "wasm32") {
            // In browser, use relative URL (same origin or proxied)
            String::new()
        } else {
            // Native, connect to local server
            "http://localhost:3000".to_string()
        };

        let async_state = Arc::new(Mutex::new(AsyncState::default()));

        let mut app = Self {
            volumes: Vec::new(),
            selected_volume: None,
            loading: true,
            error: None,
            api_base,
            async_state,
        };

        // Fetch volumes on startup
        app.fetch_volumes();

        app
    }

    fn fetch_volumes(&mut self) {
        self.loading = true;
        self.error = None;

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Native: blocking request
            let url = format!("{}/api/volumes", self.api_base);
            match reqwest::blocking::get(&url) {
                Ok(response) => match response.json::<VolumeListResponse>() {
                    Ok(data) => {
                        self.volumes = data.volumes;
                        self.loading = false;
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to parse response: {}", e));
                        self.loading = false;
                    }
                },
                Err(e) => {
                    self.error = Some(format!("Failed to fetch volumes: {}", e));
                    self.loading = false;
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            use gloo_net::http::Request;

            let state = self.async_state.clone();
            let url = format!("{}/api/volumes", self.api_base);

            wasm_bindgen_futures::spawn_local(async move {
                let result = async {
                    let response = Request::get(&url)
                        .send()
                        .await
                        .map_err(|e| format!("Request failed: {}", e))?;

                    let data: VolumeListResponse = response
                        .json()
                        .await
                        .map_err(|e| format!("Parse failed: {}", e))?;

                    Ok::<_, String>(data.volumes)
                }
                .await;

                if let Ok(mut state) = state.lock() {
                    state.volumes = Some(result);
                }
            });
        }
    }

    /// Check for async updates (WASM)
    fn poll_async_state(&mut self) {
        if let Ok(mut state) = self.async_state.lock() {
            if let Some(result) = state.volumes.take() {
                match result {
                    Ok(volumes) => {
                        self.volumes = volumes;
                        self.loading = false;
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(e);
                        self.loading = false;
                    }
                }
            }
        }
    }

    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.heading("3DLab");
        ui.separator();

        // Volume selector
        ui.label("Select Volume:");
        if self.loading {
            ui.spinner();
        } else if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::RED, error);
            if ui.button("Retry").clicked() {
                self.fetch_volumes();
            }
        } else {
            egui::ComboBox::from_label("")
                .selected_text(
                    self.selected_volume
                        .as_ref()
                        .and_then(|id| self.volumes.iter().find(|v| &v.id == id))
                        .map(|v| v.name.as_str())
                        .unwrap_or("Select..."),
                )
                .show_ui(ui, |ui| {
                    for volume in &self.volumes {
                        ui.selectable_value(
                            &mut self.selected_volume,
                            Some(volume.id.clone()),
                            &volume.name,
                        );
                    }
                });
        }

        ui.separator();

        // Volume info
        if let Some(volume) = self
            .selected_volume
            .as_ref()
            .and_then(|id| self.volumes.iter().find(|v| &v.id == id))
        {
            ui.label("Volume Info:");
            ui.label(format!(
                "Dimensions: {}x{}x{}",
                volume.dimensions[0], volume.dimensions[1], volume.dimensions[2]
            ));
            ui.label(format!(
                "Value range: {:.2} - {:.2}",
                volume.value_range[0], volume.value_range[1]
            ));
            ui.label(format!(
                "Full size: {:.1} MB",
                volume.full_res_size as f64 / 1_000_000.0
            ));
            ui.label(format!(
                "Preview size: {:.1} KB",
                volume.low_res_size as f64 / 1_000.0
            ));
        }

        ui.separator();

        // Render settings (placeholder)
        ui.collapsing("Render Settings", |ui| {
            ui.label("Quality:");
            ui.add(egui::Slider::new(&mut 0.5_f32, 0.1..=1.0).text("Step size"));

            ui.label("Transfer Function:");
            ui.add(egui::Slider::new(&mut 0.0_f32, 0.0..=1.0).text("Window"));
            ui.add(egui::Slider::new(&mut 0.5_f32, 0.0..=1.0).text("Level"));
        });

        // Bottom info
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.label("3DLab v0.1.0");
        });
    }

    fn render_viewport(&mut self, ui: &mut egui::Ui) {
        let available_size = ui.available_size();

        // Create a frame for the 3D viewport
        egui::Frame::canvas(ui.style()).show(ui, |ui| {
            let (rect, _response) = ui.allocate_exact_size(available_size, egui::Sense::drag());

            // Draw placeholder
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();

                // Background
                painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 30, 30));

                // Placeholder text
                let text = if self.selected_volume.is_some() {
                    "Volume Renderer\n(Coming Soon)"
                } else {
                    "Select a volume\nfrom the sidebar"
                };

                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::proportional(24.0),
                    egui::Color32::GRAY,
                );

                // Draw border
                painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, egui::Color32::DARK_GRAY));
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Force flat dark style every frame (overrides any persistence)
        ctx.set_style(Self::flat_style());

        // Poll for async updates
        self.poll_async_state();

        // Request repaint while loading
        if self.loading {
            ctx.request_repaint();
        }

        // Sidebar panel
        egui::SidePanel::right("sidebar")
            .resizable(true)
            .default_width(250.0)
            .show(ctx, |ui| {
                self.render_sidebar(ui);
            });

        // Main viewport
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_viewport(ui);
        });
    }
}
