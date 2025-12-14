use eframe::egui;
use eframe::glow;
use glam::Vec3;
use shared::{VolumeInfo, VolumeListResponse};
use std::sync::{Arc, Mutex};

use crate::renderer::{Camera, VolumeRenderer};

/// Shared state for async operations
#[derive(Default)]
struct AsyncState {
    volumes: Option<Result<Vec<VolumeInfo>, String>>,
    volume_data: Option<Result<VolumeData, String>>,
}

/// Loaded volume data ready for GPU upload
#[derive(Clone)]
struct VolumeData {
    data: Vec<f32>,
    dims: [u32; 3],
    value_range: [f32; 2],
}

/// Render state that can be shared across threads (no GL types)
#[derive(Clone)]
struct RenderParams {
    camera_position: Vec3,
    view_proj_matrix: glam::Mat4,
    aspect_ratio: f32,
    step_size: f32,
    value_range: [f32; 2],
    has_volume: bool,
    volume_rotation: glam::Mat4,
    show_axes: bool,
}

impl Default for RenderParams {
    fn default() -> Self {
        Self {
            camera_position: Vec3::new(0.0, 0.0, 2.0),
            view_proj_matrix: glam::Mat4::IDENTITY,
            aspect_ratio: 1.0,
            step_size: 0.005,
            value_range: [0.0, 1.0],
            has_volume: false,
            volume_rotation: glam::Mat4::IDENTITY,
            show_axes: true,
        }
    }
}

/// Shared state for the callback (no GL objects - those are created in callback)
struct SharedRenderState {
    params: RenderParams,
    pending_volume: Option<VolumeData>,
}

impl Default for SharedRenderState {
    fn default() -> Self {
        Self {
            params: RenderParams::default(),
            pending_volume: None,
        }
    }
}

/// Main application state
pub struct App {
    volumes: Vec<VolumeInfo>,
    selected_volume: Option<String>,
    loaded_volume: Option<String>,
    loading: bool,
    loading_volume: bool,
    error: Option<String>,
    api_base: String,
    async_state: Arc<Mutex<AsyncState>>,
    /// Shared render state (no GL objects)
    shared_render_state: Arc<Mutex<SharedRenderState>>,
    /// Local camera for input handling
    camera: Camera,
    /// Track if we have a volume loaded
    has_volume: bool,
    /// Volume rotation as quaternion (for trackball-style rotation)
    volume_rotation: glam::Quat,
    /// Show XYZ axis indicators
    show_axes: bool,
    /// Target resolution for volume (largest dimension)
    target_resolution: u32,
    /// Currently loaded resolution
    loaded_resolution: u32,
}

impl App {
    fn dark_visuals() -> egui::Visuals {
        let mut visuals = egui::Visuals::dark();

        visuals.window_rounding = egui::Rounding::ZERO;
        visuals.menu_rounding = egui::Rounding::ZERO;
        visuals.widgets.noninteractive.rounding = egui::Rounding::ZERO;
        visuals.widgets.inactive.rounding = egui::Rounding::ZERO;
        visuals.widgets.hovered.rounding = egui::Rounding::ZERO;
        visuals.widgets.active.rounding = egui::Rounding::ZERO;
        visuals.widgets.open.rounding = egui::Rounding::ZERO;

        visuals.widgets.hovered.expansion = 0.0;
        visuals.widgets.active.expansion = 0.0;

        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(45, 45, 45);
        visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.inactive.fg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(180, 180, 180));

        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(70, 70, 70);
        visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.hovered.fg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 220, 220));

        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(90, 90, 90);
        visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);

        visuals.widgets.open.bg_fill = egui::Color32::from_rgb(60, 60, 60);
        visuals.widgets.open.bg_stroke = egui::Stroke::NONE;

        visuals.panel_fill = egui::Color32::from_rgb(25, 25, 25);
        visuals.window_fill = egui::Color32::from_rgb(30, 30, 30);
        visuals.selection.bg_fill = egui::Color32::from_rgb(60, 80, 120);
        visuals.popup_shadow = egui::epaint::Shadow::NONE;

        visuals
    }

    fn flat_style() -> egui::Style {
        let mut style = egui::Style::default();
        style.visuals = Self::dark_visuals();
        style.spacing.button_padding = egui::vec2(4.0, 2.0);
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.combo_width = 0.0;
        style.spacing.menu_margin = egui::Margin::same(2.0);
        style.spacing.window_margin = egui::Margin::same(4.0);
        style
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_style(Self::flat_style());
        cc.egui_ctx.set_pixels_per_point(1.5);

        let api_base = if cfg!(target_arch = "wasm32") {
            String::new()
        } else {
            "http://localhost:9000".to_string()
        };

        let async_state = Arc::new(Mutex::new(AsyncState::default()));
        let shared_render_state = Arc::new(Mutex::new(SharedRenderState::default()));

        let mut app = Self {
            volumes: Vec::new(),
            selected_volume: None,
            loaded_volume: None,
            loading: true,
            loading_volume: false,
            error: None,
            api_base,
            async_state,
            shared_render_state,
            camera: Camera::default(),
            has_volume: false,
            volume_rotation: glam::Quat::IDENTITY,
            show_axes: true,
            target_resolution: 256,
            loaded_resolution: 0,
        };

        app.fetch_volumes();
        app
    }

    fn fetch_volumes(&mut self) {
        self.loading = true;
        self.error = None;

        #[cfg(not(target_arch = "wasm32"))]
        {
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

    fn fetch_volume_data(&mut self, volume_id: &str) {
        self.loading_volume = true;

        let volume_info = self.volumes.iter().find(|v| v.id == volume_id).cloned();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let url = format!("{}/api/volumes/{}/full", self.api_base, volume_id);
            match reqwest::blocking::get(&url) {
                Ok(response) => match response.bytes() {
                    Ok(bytes) => {
                        if let Some(info) = volume_info {
                            let data: Vec<f32> = bytes
                                .chunks_exact(4)
                                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                                .collect();

                            if let Ok(mut state) = self.async_state.lock() {
                                state.volume_data = Some(Ok(VolumeData {
                                    data,
                                    dims: info.dimensions,
                                    value_range: info.value_range,
                                }));
                            }
                        }
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to read volume data: {}", e));
                        self.loading_volume = false;
                    }
                },
                Err(e) => {
                    self.error = Some(format!("Failed to fetch volume: {}", e));
                    self.loading_volume = false;
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            use gloo_net::http::Request;

            let state = self.async_state.clone();
            let url = format!("{}/api/volumes/{}/full", self.api_base, volume_id);

            wasm_bindgen_futures::spawn_local(async move {
                let result = async {
                    let response = Request::get(&url)
                        .send()
                        .await
                        .map_err(|e| format!("Request failed: {}", e))?;

                    let bytes = response
                        .binary()
                        .await
                        .map_err(|e| format!("Failed to read bytes: {}", e))?;

                    let data: Vec<f32> = bytes
                        .chunks_exact(4)
                        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                        .collect();

                    let (dims, value_range) = if let Some(info) = volume_info {
                        (info.dimensions, info.value_range)
                    } else {
                        // Fallback: try to infer cubic dimensions from data length
                        let side = (data.len() as f32).cbrt().round() as u32;
                        ([side, side, side], [0.0, 1.0])
                    };

                    Ok::<_, String>(VolumeData {
                        data,
                        dims,
                        value_range,
                    })
                }
                .await;

                if let Ok(mut state) = state.lock() {
                    state.volume_data = Some(result);
                }
            });
        }
    }

    fn fetch_volume_at_resolution(&mut self, volume_id: &str, resolution: u32) {
        self.loading_volume = true;

        let volume_info = self.volumes.iter().find(|v| v.id == volume_id).cloned();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let url = format!("{}/api/volumes/{}/at/{}", self.api_base, volume_id, resolution);
            match reqwest::blocking::get(&url) {
                Ok(response) => {
                    // Get dimensions from header
                    let dims: [u32; 3] = response
                        .headers()
                        .get("x-volume-dims")
                        .and_then(|h| h.to_str().ok())
                        .map(|s| {
                            let parts: Vec<u32> = s.split(',').filter_map(|p| p.parse().ok()).collect();
                            if parts.len() == 3 {
                                [parts[0], parts[1], parts[2]]
                            } else {
                                [resolution, resolution, resolution]
                            }
                        })
                        .unwrap_or([resolution, resolution, resolution]);

                    match response.bytes() {
                        Ok(bytes) => {
                            let data: Vec<f32> = bytes
                                .chunks_exact(4)
                                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                                .collect();

                            let value_range = volume_info.map(|i| i.value_range).unwrap_or([0.0, 1.0]);

                            if let Ok(mut state) = self.async_state.lock() {
                                state.volume_data = Some(Ok(VolumeData {
                                    data,
                                    dims,
                                    value_range,
                                }));
                            }
                        }
                        Err(e) => {
                            self.error = Some(format!("Failed to read volume data: {}", e));
                            self.loading_volume = false;
                        }
                    }
                }
                Err(e) => {
                    self.error = Some(format!("Failed to fetch volume: {}", e));
                    self.loading_volume = false;
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            use gloo_net::http::Request;

            let state = self.async_state.clone();
            let url = format!("{}/api/volumes/{}/at/{}", self.api_base, volume_id, resolution);
            let value_range = volume_info.map(|i| i.value_range).unwrap_or([0.0, 1.0]);

            wasm_bindgen_futures::spawn_local(async move {
                let result = async {
                    let response = Request::get(&url)
                        .send()
                        .await
                        .map_err(|e| format!("Request failed: {}", e))?;

                    // Get dimensions from header
                    let dims: [u32; 3] = response
                        .headers()
                        .get("x-volume-dims")
                        .map(|s| {
                            let parts: Vec<u32> = s.split(',').filter_map(|p| p.parse().ok()).collect();
                            if parts.len() == 3 {
                                [parts[0], parts[1], parts[2]]
                            } else {
                                [resolution, resolution, resolution]
                            }
                        })
                        .unwrap_or([resolution, resolution, resolution]);

                    let bytes = response
                        .binary()
                        .await
                        .map_err(|e| format!("Failed to read bytes: {}", e))?;

                    let data: Vec<f32> = bytes
                        .chunks_exact(4)
                        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                        .collect();

                    Ok::<_, String>(VolumeData {
                        data,
                        dims,
                        value_range,
                    })
                }
                .await;

                if let Ok(mut state) = state.lock() {
                    state.volume_data = Some(result);
                }
            });
        }
    }

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

            if let Some(result) = state.volume_data.take() {
                match result {
                    Ok(data) => {
                        self.loading_volume = false;
                        // Store pending volume in shared state for callback to pick up
                        if let Ok(mut render_state) = self.shared_render_state.lock() {
                            render_state.params.value_range = data.value_range;
                            render_state.pending_volume = Some(data);
                        }
                        self.has_volume = true;
                        self.loaded_volume = self.selected_volume.clone();
                    }
                    Err(e) => {
                        self.error = Some(e);
                        self.loading_volume = false;
                    }
                }
            }
        }
    }

    fn render_sidebar(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let mut volume_changed = None;

        ui.heading("3DLab");
        ui.separator();

        ui.label("Select Volume:");
        if self.loading {
            ui.spinner();
        } else if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::RED, error);
            if ui.button("Retry").clicked() {
                self.fetch_volumes();
            }
        } else {
            let previous_selection = self.selected_volume.clone();

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

            if self.selected_volume != previous_selection {
                volume_changed = self.selected_volume.clone();
            }
        }

        ui.separator();

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

            if self.loading_volume {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Loading volume...");
                });
            }
        }

        ui.separator();

        // Rotation controls (Euler angles derived from quaternion)
        ui.label("Rotation:");

        // Derive Euler angles from quaternion for display
        let (euler_x, euler_y, euler_z) = self.volume_rotation.to_euler(glam::EulerRot::XYZ);
        let mut euler_deg = [
            euler_x.to_degrees(),
            euler_y.to_degrees(),
            euler_z.to_degrees(),
        ];

        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("X:");
            changed |= ui.add(egui::DragValue::new(&mut euler_deg[0]).speed(1.0).suffix("°")).changed();
        });
        ui.horizontal(|ui| {
            ui.label("Y:");
            changed |= ui.add(egui::DragValue::new(&mut euler_deg[1]).speed(1.0).suffix("°")).changed();
        });
        ui.horizontal(|ui| {
            ui.label("Z:");
            changed |= ui.add(egui::DragValue::new(&mut euler_deg[2]).speed(1.0).suffix("°")).changed();
        });

        // If slider changed, convert back to quaternion
        if changed {
            self.volume_rotation = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                euler_deg[0].to_radians(),
                euler_deg[1].to_radians(),
                euler_deg[2].to_radians(),
            );
        }

        if ui.button("Reset Rotation").clicked() {
            self.volume_rotation = glam::Quat::IDENTITY;
        }

        ui.separator();

        // Axes toggle
        ui.checkbox(&mut self.show_axes, "Show Axes");

        ui.separator();

        // Resolution slider
        ui.label("Resolution:");
        let old_resolution = self.target_resolution;
        ui.add(egui::Slider::new(&mut self.target_resolution, 32..=512).text("px"));
        if old_resolution != self.target_resolution && !self.loading_volume {
            // Resolution changed, trigger reload if we have a volume selected
            if let Some(ref id) = self.selected_volume {
                volume_changed = Some(format!("{}@{}", id, self.target_resolution));
            }
        }

        ui.separator();

        volume_changed
    }

    fn render_footer(&self, ui: &mut egui::Ui) {
        let fps = 1.0 / ui.ctx().input(|i| i.stable_dt).max(0.001);
        ui.label(format!("{:.0} fps", fps));
        ui.label("3DLab v0.1.0");
    }

    fn render_viewport(&mut self, ui: &mut egui::Ui, _gl: &Arc<glow::Context>) {
        let available_size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());

        let aspect_ratio = rect.width() / rect.height();

        // Handle drag for volume rotation (trackball-style, view-relative)
        if response.dragged() {
            let delta = response.drag_delta();
            let sensitivity = 0.01;  // radians per pixel

            // Rotate around screen axes (Y for horizontal drag, X for vertical drag)
            let rot_y = glam::Quat::from_axis_angle(glam::Vec3::Y, delta.x * sensitivity);
            let rot_x = glam::Quat::from_axis_angle(glam::Vec3::X, delta.y * sensitivity);

            // Apply incremental rotation: new = delta * current
            self.volume_rotation = (rot_y * rot_x) * self.volume_rotation;
            self.volume_rotation = self.volume_rotation.normalize();
        }

        // Handle zoom with scroll
        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if scroll_delta != 0.0 {
            self.camera.zoom(scroll_delta * 0.01);
        }

        // Build volume rotation matrix from quaternion
        let volume_rotation = glam::Mat4::from_quat(self.volume_rotation);

        // Update shared render state with camera params
        if let Ok(mut state) = self.shared_render_state.lock() {
            state.params.camera_position = self.camera.position();
            state.params.view_proj_matrix = self.camera.view_projection_matrix(aspect_ratio);
            state.params.aspect_ratio = aspect_ratio;
            state.params.has_volume = self.has_volume;
            state.params.volume_rotation = volume_rotation;
            state.params.show_axes = self.show_axes;
        }

        if !self.has_volume {
            // Draw placeholder
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 30, 30));

            let text = if self.loading_volume {
                "Loading volume..."
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
        } else {
            // Custom OpenGL rendering callback
            // The callback owns the renderer (created lazily) and reads params from shared state
            let shared_state = self.shared_render_state.clone();

            let callback = egui::PaintCallback {
                rect,
                callback: Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                    // Use thread-local storage for the renderer since it can't be shared
                    use std::cell::RefCell;
                    thread_local! {
                        static RENDERER: RefCell<Option<VolumeRenderer>> = const { RefCell::new(None) };
                    }

                    RENDERER.with(|renderer_cell| {
                        let mut renderer_opt = renderer_cell.borrow_mut();

                        // Create renderer lazily
                        if renderer_opt.is_none() {
                            *renderer_opt = Some(VolumeRenderer::new(painter.gl()));
                        }

                        if let Some(ref mut renderer) = *renderer_opt {
                            // Get params and pending volume from shared state
                            if let Ok(mut state) = shared_state.lock() {
                                // Upload pending volume if any
                                if let Some(vol_data) = state.pending_volume.take() {
                                    renderer.upload_volume(
                                        painter.gl(),
                                        &vol_data.data,
                                        vol_data.dims,
                                        vol_data.value_range,
                                    );
                                }

                                // Render if we have a volume
                                if state.params.has_volume && renderer.has_volume() {
                                    renderer.render_with_params(
                                        painter.gl(),
                                        &state.params.view_proj_matrix,
                                        &state.params.camera_position,
                                        state.params.step_size,
                                        state.params.value_range,
                                        &state.params.volume_rotation,
                                    );

                                    // Render axes if enabled
                                    if state.params.show_axes {
                                        renderer.render_axes(
                                            painter.gl(),
                                            &state.params.view_proj_matrix,
                                            &state.params.volume_rotation,
                                        );
                                    }
                                }
                            }
                        }
                    });
                })),
            };
            ui.painter().add(callback);
        }

        // Request continuous repaint for smooth interaction
        ui.ctx().request_repaint();
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        ctx.set_style(Self::flat_style());

        self.poll_async_state();

        if self.loading || self.loading_volume {
            ctx.request_repaint();
        }

        // Get GL context from frame
        let gl = frame.gl().cloned();

        let mut volume_to_fetch: Option<String> = None;

        egui::SidePanel::right("sidebar")
            .resizable(true)
            .default_width(250.0)
            .show(ctx, |ui| {
                if let Some(new_volume) = self.render_sidebar(ui) {
                    volume_to_fetch = Some(new_volume);
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    self.render_footer(ui);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(ref gl) = gl {
                self.render_viewport(ui, gl);
            } else {
                // No GL context available
                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 30, 30));
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "OpenGL not available",
                    egui::FontId::proportional(24.0),
                    egui::Color32::RED,
                );
            }
        });

        if let Some(volume_request) = volume_to_fetch {
            // Check if this is a resolution change (format: "volume_id@resolution")
            if let Some((volume_id, resolution_str)) = volume_request.split_once('@') {
                if let Ok(resolution) = resolution_str.parse::<u32>() {
                    // Resolution change request
                    self.fetch_volume_at_resolution(volume_id, resolution);
                    self.loaded_resolution = resolution;
                }
            } else {
                // Normal volume selection
                let volume_id = volume_request;
                if self.loaded_volume.as_ref() != Some(&volume_id) {
                    self.fetch_volume_at_resolution(&volume_id, self.target_resolution);
                    self.loaded_resolution = self.target_resolution;
                }
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&glow::Context>) {
        // Renderer cleanup is handled by thread-local storage drop
        // when the thread exits
    }
}
