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

/// Info about a point in the volume (for hover display)
#[derive(Clone, Default)]
struct HoverInfo {
    /// Whether we have valid hover info
    valid: bool,
    /// Position in volume space [0,1]
    position: [f32; 3],
    /// Raw voxel value
    value: f32,
    /// Normalized value [0,1]
    normalized: f32,
    /// Voxel coordinates
    voxel: [u32; 3],
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
    opacity: f32,
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
            opacity: 1.0,
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
    /// Euler angles in degrees (for slider display, synced with quaternion)
    volume_euler_deg: [f32; 3],
    /// Show XYZ axis indicators
    show_axes: bool,
    /// Render quality (0.0 = fast/low, 1.0 = slow/high)
    render_quality: f32,
    /// Volume opacity (0.0 = transparent, 1.0 = opaque)
    opacity: f32,
    /// CPU copy of volume data for hover raycasting
    cpu_volume_data: Option<VolumeData>,
    /// Current hover info
    hover_info: HoverInfo,
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

        // Use monospace font for everything
        use egui::{FontFamily, FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Small, FontId::new(10.0, FontFamily::Monospace)),
            (TextStyle::Body, FontId::new(13.0, FontFamily::Monospace)),
            (TextStyle::Button, FontId::new(13.0, FontFamily::Monospace)),
            (TextStyle::Heading, FontId::new(18.0, FontFamily::Monospace)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ].into();

        style
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_style(Self::flat_style());

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
            volume_euler_deg: [0.0, 0.0, 0.0],
            show_axes: true,
            render_quality: 0.5,  // Default to medium quality
            opacity: 1.0,  // Default to fully opaque
            cpu_volume_data: None,
            hover_info: HoverInfo::default(),
        };

        app.fetch_volumes();
        app
    }

    fn fetch_volumes(&mut self) {
        self.loading = true;
        self.error = None;

        #[cfg(not(target_arch = "wasm32"))]
        {
            let state = self.async_state.clone();
            let url = format!("{}/api/volumes", self.api_base);

            // Spawn background thread to avoid blocking render loop
            std::thread::spawn(move || {
                let result = match reqwest::blocking::get(&url) {
                    Ok(response) => match response.json::<VolumeListResponse>() {
                        Ok(data) => Ok(data.volumes),
                        Err(e) => Err(format!("Failed to parse response: {}", e)),
                    },
                    Err(e) => Err(format!("Failed to fetch volumes: {}", e)),
                };

                if let Ok(mut state) = state.lock() {
                    state.volumes = Some(result);
                }
            });
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
            let state = self.async_state.clone();
            let url = format!("{}/api/volumes/{}/full", self.api_base, volume_id);

            // Spawn background thread to avoid blocking render loop
            std::thread::spawn(move || {
                let result = match reqwest::blocking::get(&url) {
                    Ok(response) => match response.bytes() {
                        Ok(bytes) => {
                            if let Some(info) = volume_info {
                                let data: Vec<f32> = bytes
                                    .chunks_exact(4)
                                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                                    .collect();

                                Ok(VolumeData {
                                    data,
                                    dims: info.dimensions,
                                    value_range: info.value_range,
                                })
                            } else {
                                Err("Volume info not found".to_string())
                            }
                        }
                        Err(e) => Err(format!("Failed to read volume data: {}", e)),
                    },
                    Err(e) => Err(format!("Failed to fetch volume: {}", e)),
                };

                if let Ok(mut state) = state.lock() {
                    state.volume_data = Some(result);
                }
            });
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
                        // Keep a CPU copy for hover raycasting
                        self.cpu_volume_data = Some(data.clone());
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

    /// Raycast into volume to find first significant voxel
    fn raycast_volume(&self, ray_origin: Vec3, ray_dir: Vec3) -> Option<HoverInfo> {
        let vol_data = self.cpu_volume_data.as_ref()?;
        let dims = vol_data.dims;
        let value_range = vol_data.value_range;

        // Ray-box intersection for unit cube [0,1]³
        let inv_dir = Vec3::new(1.0 / ray_dir.x, 1.0 / ray_dir.y, 1.0 / ray_dir.z);
        let t0 = (Vec3::ZERO - ray_origin) * inv_dir;
        let t1 = (Vec3::ONE - ray_origin) * inv_dir;
        let t_min = t0.min(t1);
        let t_max = t0.max(t1);
        let t_near = t_min.x.max(t_min.y).max(t_min.z).max(0.0);
        let t_far = t_max.x.min(t_max.y).min(t_max.z);

        if t_near > t_far {
            return None;
        }

        // March through volume
        let step_size = 0.01;
        let mut t = t_near;
        let threshold = 0.05; // Normalized threshold for "significant" voxel

        while t < t_far {
            let pos = ray_origin + ray_dir * t;

            // Apply inverse rotation
            let centered = pos - Vec3::new(0.5, 0.5, 0.5);
            let rot_inv = glam::Mat4::from_quat(self.volume_rotation).transpose();
            let rotated = rot_inv.transform_point3(centered);
            let rotated_pos = rotated + Vec3::new(0.5, 0.5, 0.5);

            // Check bounds
            if rotated_pos.x >= 0.0 && rotated_pos.x <= 1.0 &&
               rotated_pos.y >= 0.0 && rotated_pos.y <= 1.0 &&
               rotated_pos.z >= 0.0 && rotated_pos.z <= 1.0 {

                // Sample volume (trilinear approximation - just use nearest for simplicity)
                let vx = ((rotated_pos.x * dims[0] as f32) as u32).min(dims[0] - 1);
                let vy = ((rotated_pos.y * dims[1] as f32) as u32).min(dims[1] - 1);
                let vz = ((rotated_pos.z * dims[2] as f32) as u32).min(dims[2] - 1);

                let idx = (vx * dims[1] * dims[2] + vy * dims[2] + vz) as usize;
                if idx < vol_data.data.len() {
                    let value = vol_data.data[idx];
                    let normalized = (value - value_range[0]) / (value_range[1] - value_range[0]);

                    if normalized > threshold {
                        return Some(HoverInfo {
                            valid: true,
                            position: [rotated_pos.x, rotated_pos.y, rotated_pos.z],
                            value,
                            normalized,
                            voxel: [vx, vy, vz],
                        });
                    }
                }
            }

            t += step_size;
        }

        None
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

        // Rotation controls (Euler angles synced with quaternion)
        ui.label("Rotation:");

        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("X:");
            changed |= ui.add(egui::DragValue::new(&mut self.volume_euler_deg[0]).speed(1.0).suffix("°")).changed();
        });
        ui.horizontal(|ui| {
            ui.label("Y:");
            changed |= ui.add(egui::DragValue::new(&mut self.volume_euler_deg[1]).speed(1.0).suffix("°")).changed();
        });
        ui.horizontal(|ui| {
            ui.label("Z:");
            changed |= ui.add(egui::DragValue::new(&mut self.volume_euler_deg[2]).speed(1.0).suffix("°")).changed();
        });

        // If slider changed, update quaternion from Euler
        if changed {
            self.volume_rotation = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                self.volume_euler_deg[0].to_radians(),
                self.volume_euler_deg[1].to_radians(),
                self.volume_euler_deg[2].to_radians(),
            );
        }

        if ui.button("Reset Rotation").clicked() {
            self.volume_rotation = glam::Quat::IDENTITY;
            self.volume_euler_deg = [0.0, 0.0, 0.0];
        }

        ui.separator();

        // Axes toggle
        ui.checkbox(&mut self.show_axes, "Show Axes");

        ui.separator();

        // Quality slider (affects render performance)
        ui.label("Quality:");
        ui.add(egui::Slider::new(&mut self.render_quality, 0.0..=1.0).text(""));
        ui.label(egui::RichText::new("(lower = faster)").small().weak());

        ui.separator();

        // Opacity slider
        ui.label("Opacity:");
        ui.add(egui::Slider::new(&mut self.opacity, 0.0..=1.0).text(""));

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

            // Sync Euler angles from quaternion for slider display
            let (ex, ey, ez) = self.volume_rotation.to_euler(glam::EulerRot::XYZ);
            self.volume_euler_deg = [ex.to_degrees(), ey.to_degrees(), ez.to_degrees()];
        }

        // Handle zoom with scroll
        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if scroll_delta != 0.0 {
            self.camera.zoom(scroll_delta * 0.01);
        }

        // Handle hover for voxel info
        if response.hovered() && self.has_volume && self.cpu_volume_data.is_some() {
            if let Some(hover_pos) = response.hover_pos() {
                // Convert screen position to normalized device coordinates
                let ndc_x = (hover_pos.x - rect.center().x) / (rect.width() * 0.5);
                let ndc_y = -(hover_pos.y - rect.center().y) / (rect.height() * 0.5);

                // Get inverse view-projection matrix
                let view_proj = self.camera.view_projection_matrix(aspect_ratio);
                let inv_view_proj = view_proj.inverse();

                // Unproject to world space
                let near_point = inv_view_proj.project_point3(glam::Vec3::new(ndc_x, ndc_y, -1.0));
                let far_point = inv_view_proj.project_point3(glam::Vec3::new(ndc_x, ndc_y, 1.0));

                // Ray in world space (shifted to volume [0,1] space)
                let ray_origin = near_point + Vec3::new(0.5, 0.5, 0.5);
                let ray_dir = (far_point - near_point).normalize();

                // Raycast into volume
                if let Some(info) = self.raycast_volume(ray_origin, ray_dir) {
                    self.hover_info = info;
                } else {
                    self.hover_info.valid = false;
                }
            }
        } else {
            self.hover_info.valid = false;
        }

        // Build volume rotation matrix from quaternion
        let volume_rotation = glam::Mat4::from_quat(self.volume_rotation);

        // Compute step_size from quality (0.0 = fast/large steps, 1.0 = slow/small steps)
        // Range: 0.02 (fast) to 0.003 (high quality)
        let step_size = 0.02 - (self.render_quality * 0.017);

        // Update shared render state with camera params
        if let Ok(mut state) = self.shared_render_state.lock() {
            state.params.camera_position = self.camera.position();
            state.params.view_proj_matrix = self.camera.view_projection_matrix(aspect_ratio);
            state.params.aspect_ratio = aspect_ratio;
            state.params.has_volume = self.has_volume;
            state.params.volume_rotation = volume_rotation;
            state.params.show_axes = self.show_axes;
            state.params.step_size = step_size;
            state.params.opacity = self.opacity;
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
                egui::FontId::monospace(20.0),
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
                                        state.params.opacity,
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

            // Show hover info overlay
            if self.hover_info.valid {
                let info = &self.hover_info;

                // Position the info panel in the top-left corner of the viewport
                let panel_pos = rect.left_top() + egui::vec2(10.0, 10.0);

                let panel_rect = egui::Rect::from_min_size(
                    panel_pos,
                    egui::vec2(160.0, 80.0),
                );

                // Draw background
                ui.painter().rect_filled(
                    panel_rect,
                    4.0,
                    egui::Color32::from_rgba_unmultiplied(20, 20, 20, 220),
                );

                // Draw info text
                let text_pos = panel_pos + egui::vec2(8.0, 8.0);
                let line_height = 16.0;

                ui.painter().text(
                    text_pos,
                    egui::Align2::LEFT_TOP,
                    format!("Voxel: ({}, {}, {})", info.voxel[0], info.voxel[1], info.voxel[2]),
                    egui::FontId::monospace(12.0),
                    egui::Color32::WHITE,
                );

                ui.painter().text(
                    text_pos + egui::vec2(0.0, line_height),
                    egui::Align2::LEFT_TOP,
                    format!("Value: {:.4}", info.value),
                    egui::FontId::monospace(12.0),
                    egui::Color32::WHITE,
                );

                ui.painter().text(
                    text_pos + egui::vec2(0.0, line_height * 2.0),
                    egui::Align2::LEFT_TOP,
                    format!("Intensity: {:.1}%", info.normalized * 100.0),
                    egui::FontId::monospace(12.0),
                    egui::Color32::WHITE,
                );

                ui.painter().text(
                    text_pos + egui::vec2(0.0, line_height * 3.0),
                    egui::Align2::LEFT_TOP,
                    format!("Pos: ({:.2}, {:.2}, {:.2})", info.position[0], info.position[1], info.position[2]),
                    egui::FontId::monospace(12.0),
                    egui::Color32::GRAY,
                );
            }
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
                    egui::FontId::monospace(20.0),
                    egui::Color32::RED,
                );
            }
        });

        if let Some(volume_id) = volume_to_fetch {
            if self.loaded_volume.as_ref() != Some(&volume_id) {
                // Always load at full resolution
                self.fetch_volume_data(&volume_id);
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&glow::Context>) {
        // Renderer cleanup is handled by thread-local storage drop
        // when the thread exits
    }
}
