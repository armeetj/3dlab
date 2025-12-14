use glow::HasContext;

use super::camera::Camera;

const VERTEX_SHADER: &str = include_str!("shaders/volume.vert");
const FRAGMENT_SHADER: &str = include_str!("shaders/volume.frag");
const AXES_VERTEX_SHADER: &str = include_str!("shaders/axes.vert");
const AXES_FRAGMENT_SHADER: &str = include_str!("shaders/axes.frag");

/// Size of the occupancy grid (cells per dimension)
const OCCUPANCY_GRID_SIZE: u32 = 16;

/// Volume renderer using OpenGL ray marching
pub struct VolumeRenderer {
    program: glow::Program,
    vao: glow::VertexArray,
    volume_texture: Option<glow::Texture>,
    occupancy_texture: Option<glow::Texture>,
    pub camera: Camera,
    volume_dims: [u32; 3],
    value_range: [f32; 2],
    // Uniform locations
    u_view_proj: Option<glow::UniformLocation>,
    u_camera_pos: Option<glow::UniformLocation>,
    u_step_size: Option<glow::UniformLocation>,
    u_value_min: Option<glow::UniformLocation>,
    u_value_max: Option<glow::UniformLocation>,
    u_volume: Option<glow::UniformLocation>,
    u_volume_rotation: Option<glow::UniformLocation>,
    u_occupancy: Option<glow::UniformLocation>,
    u_occupancy_size: Option<glow::UniformLocation>,
    u_opacity: Option<glow::UniformLocation>,
    // Axes rendering
    axes_program: glow::Program,
    axes_vao: glow::VertexArray,
    axes_vbo: glow::Buffer,
    axes_u_view_proj: Option<glow::UniformLocation>,
    axes_u_model: Option<glow::UniformLocation>,
}

impl VolumeRenderer {
    pub fn new(gl: &glow::Context) -> Self {
        unsafe {
            // Compile shaders
            let vertex_shader = gl.create_shader(glow::VERTEX_SHADER).unwrap();
            gl.shader_source(vertex_shader, VERTEX_SHADER);
            gl.compile_shader(vertex_shader);
            if !gl.get_shader_compile_status(vertex_shader) {
                panic!("Vertex shader error: {}", gl.get_shader_info_log(vertex_shader));
            }

            let fragment_shader = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
            gl.shader_source(fragment_shader, FRAGMENT_SHADER);
            gl.compile_shader(fragment_shader);
            if !gl.get_shader_compile_status(fragment_shader) {
                panic!("Fragment shader error: {}", gl.get_shader_info_log(fragment_shader));
            }

            // Link program
            let program = gl.create_program().unwrap();
            gl.attach_shader(program, vertex_shader);
            gl.attach_shader(program, fragment_shader);
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                panic!("Program link error: {}", gl.get_program_info_log(program));
            }

            // Clean up shaders
            gl.delete_shader(vertex_shader);
            gl.delete_shader(fragment_shader);

            // Get uniform locations
            let u_view_proj = gl.get_uniform_location(program, "u_view_proj");
            let u_camera_pos = gl.get_uniform_location(program, "u_camera_pos");
            let u_step_size = gl.get_uniform_location(program, "u_step_size");
            let u_value_min = gl.get_uniform_location(program, "u_value_min");
            let u_value_max = gl.get_uniform_location(program, "u_value_max");
            let u_volume = gl.get_uniform_location(program, "u_volume");
            let u_volume_rotation = gl.get_uniform_location(program, "u_volume_rotation");
            let u_occupancy = gl.get_uniform_location(program, "u_occupancy");
            let u_occupancy_size = gl.get_uniform_location(program, "u_occupancy_size");
            let u_opacity = gl.get_uniform_location(program, "u_opacity");

            // Create VAO (required for WebGL2/OpenGL ES 3.0)
            let vao = gl.create_vertex_array().unwrap();

            // === Axes shader setup ===
            let axes_vs = gl.create_shader(glow::VERTEX_SHADER).unwrap();
            gl.shader_source(axes_vs, AXES_VERTEX_SHADER);
            gl.compile_shader(axes_vs);
            if !gl.get_shader_compile_status(axes_vs) {
                panic!("Axes vertex shader error: {}", gl.get_shader_info_log(axes_vs));
            }

            let axes_fs = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
            gl.shader_source(axes_fs, AXES_FRAGMENT_SHADER);
            gl.compile_shader(axes_fs);
            if !gl.get_shader_compile_status(axes_fs) {
                panic!("Axes fragment shader error: {}", gl.get_shader_info_log(axes_fs));
            }

            let axes_program = gl.create_program().unwrap();
            gl.attach_shader(axes_program, axes_vs);
            gl.attach_shader(axes_program, axes_fs);
            gl.link_program(axes_program);
            if !gl.get_program_link_status(axes_program) {
                panic!("Axes program link error: {}", gl.get_program_info_log(axes_program));
            }

            gl.delete_shader(axes_vs);
            gl.delete_shader(axes_fs);

            let axes_u_view_proj = gl.get_uniform_location(axes_program, "u_view_proj");
            let axes_u_model = gl.get_uniform_location(axes_program, "u_model");

            // Create axes vertex data: 6 vertices (2 per axis), each with position + color
            // Position (3 floats) + Color (3 floats) = 6 floats per vertex
            // X axis: Red (1,0,0), Y axis: Green (0,1,0), Z axis: Blue (0,0,1)
            let axis_length = 0.3;
            let axes_vertices: [f32; 36] = [
                // X axis (red)
                0.0, 0.0, 0.0, 1.0, 0.0, 0.0,  // origin
                axis_length, 0.0, 0.0, 1.0, 0.0, 0.0,  // +X
                // Y axis (green)
                0.0, 0.0, 0.0, 0.0, 1.0, 0.0,  // origin
                0.0, axis_length, 0.0, 0.0, 1.0, 0.0,  // +Y
                // Z axis (blue)
                0.0, 0.0, 0.0, 0.0, 0.0, 1.0,  // origin
                0.0, 0.0, axis_length, 0.0, 0.0, 1.0,  // +Z
            ];

            let axes_vao = gl.create_vertex_array().unwrap();
            let axes_vbo = gl.create_buffer().unwrap();

            gl.bind_vertex_array(Some(axes_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(axes_vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&axes_vertices),
                glow::STATIC_DRAW,
            );

            // Position attribute (location 0)
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 24, 0);

            // Color attribute (location 1)
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 24, 12);

            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);

            Self {
                program,
                vao,
                volume_texture: None,
                occupancy_texture: None,
                camera: Camera::default(),
                volume_dims: [1, 1, 1],
                value_range: [0.0, 1.0],
                u_view_proj,
                u_camera_pos,
                u_step_size,
                u_value_min,
                u_value_max,
                u_volume,
                u_volume_rotation,
                u_occupancy,
                u_occupancy_size,
                u_opacity,
                axes_program,
                axes_vao,
                axes_vbo,
                axes_u_view_proj,
                axes_u_model,
            }
        }
    }

    /// Upload volume data as a 3D texture
    pub fn upload_volume(
        &mut self,
        gl: &glow::Context,
        data: &[f32],
        dims: [u32; 3],
        value_range: [f32; 2],
    ) {
        self.volume_dims = dims;
        self.value_range = value_range;

        unsafe {
            // Delete old textures if they exist
            if let Some(tex) = self.volume_texture.take() {
                gl.delete_texture(tex);
            }
            if let Some(tex) = self.occupancy_texture.take() {
                gl.delete_texture(tex);
            }

            // Create 3D texture for volume
            let texture = gl.create_texture().unwrap();
            gl.bind_texture(glow::TEXTURE_3D, Some(texture));

            // Set texture parameters
            gl.tex_parameter_i32(glow::TEXTURE_3D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_3D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_3D, glow::TEXTURE_WRAP_R, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_3D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_3D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);

            // Convert f32 to bytes
            let bytes: &[u8] = bytemuck::cast_slice(data);

            // Upload texture data
            // Note: dims from server are [X, Y, Z] but data is row-major with Z varying fastest
            // OpenGL expects width (fastest) first, so we swap: [Z, Y, X]
            gl.tex_image_3d(
                glow::TEXTURE_3D,
                0,
                glow::R32F as i32,
                dims[2] as i32,  // width = Z (fastest varying in memory)
                dims[1] as i32,  // height = Y
                dims[0] as i32,  // depth = X (slowest varying in memory)
                0,
                glow::RED,
                glow::FLOAT,
                Some(bytes),
            );

            gl.bind_texture(glow::TEXTURE_3D, None);
            self.volume_texture = Some(texture);

            // Compute and upload occupancy grid
            let occupancy = Self::compute_occupancy_grid(data, dims, value_range);
            let occ_texture = gl.create_texture().unwrap();
            gl.bind_texture(glow::TEXTURE_3D, Some(occ_texture));

            gl.tex_parameter_i32(glow::TEXTURE_3D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_3D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_3D, glow::TEXTURE_WRAP_R, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_3D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
            gl.tex_parameter_i32(glow::TEXTURE_3D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);

            let occ_bytes: &[u8] = bytemuck::cast_slice(&occupancy);
            gl.tex_image_3d(
                glow::TEXTURE_3D,
                0,
                glow::R32F as i32,
                OCCUPANCY_GRID_SIZE as i32,
                OCCUPANCY_GRID_SIZE as i32,
                OCCUPANCY_GRID_SIZE as i32,
                0,
                glow::RED,
                glow::FLOAT,
                Some(occ_bytes),
            );

            gl.bind_texture(glow::TEXTURE_3D, None);
            self.occupancy_texture = Some(occ_texture);
        }
    }

    /// Compute occupancy grid from volume data
    /// Returns a 3D grid where each cell is 1.0 if that region has data above threshold, 0.0 otherwise
    fn compute_occupancy_grid(data: &[f32], dims: [u32; 3], value_range: [f32; 2]) -> Vec<f32> {
        let grid_size = OCCUPANCY_GRID_SIZE as usize;
        let mut occupancy = vec![0.0f32; grid_size * grid_size * grid_size];

        // Threshold: consider occupied if normalized value > 0.02
        let threshold = value_range[0] + (value_range[1] - value_range[0]) * 0.02;

        // Size of each grid cell in volume voxels
        let cell_size_x = (dims[0] as f32) / (grid_size as f32);
        let cell_size_y = (dims[1] as f32) / (grid_size as f32);
        let cell_size_z = (dims[2] as f32) / (grid_size as f32);

        // For each voxel, mark its corresponding occupancy cell
        for x in 0..dims[0] {
            for y in 0..dims[1] {
                for z in 0..dims[2] {
                    // Volume data index (row-major, Z fastest)
                    let vol_idx = (x * dims[1] * dims[2] + y * dims[2] + z) as usize;
                    if vol_idx >= data.len() {
                        continue;
                    }

                    let value = data[vol_idx];
                    if value > threshold {
                        // Map to occupancy grid cell
                        let ox = ((x as f32) / cell_size_x).min((grid_size - 1) as f32) as usize;
                        let oy = ((y as f32) / cell_size_y).min((grid_size - 1) as f32) as usize;
                        let oz = ((z as f32) / cell_size_z).min((grid_size - 1) as f32) as usize;

                        // Occupancy grid index (same layout as volume)
                        let occ_idx = ox * grid_size * grid_size + oy * grid_size + oz;
                        occupancy[occ_idx] = 1.0;
                    }
                }
            }
        }

        occupancy
    }

    /// Check if volume data is loaded
    pub fn has_volume(&self) -> bool {
        self.volume_texture.is_some()
    }

    /// Render the volume using internal camera
    pub fn render(&self, gl: &glow::Context, aspect_ratio: f32) {
        let view_proj = self.camera.view_projection_matrix(aspect_ratio);
        let camera_pos = self.camera.position();
        self.render_with_params(gl, &view_proj, &camera_pos, 0.005, self.value_range, &glam::Mat4::IDENTITY, 1.0);
    }

    /// Render the volume with explicit parameters
    pub fn render_with_params(
        &self,
        gl: &glow::Context,
        view_proj: &glam::Mat4,
        camera_pos: &glam::Vec3,
        step_size: f32,
        value_range: [f32; 2],
        volume_rotation: &glam::Mat4,
        opacity: f32,
    ) {
        if self.volume_texture.is_none() {
            return;
        }

        unsafe {
            // Enable blending and cull face
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.enable(glow::CULL_FACE);
            gl.cull_face(glow::FRONT); // Cull front faces for inside-out rendering

            // Use our program
            gl.use_program(Some(self.program));
            gl.bind_vertex_array(Some(self.vao));

            // Set uniforms
            if let Some(loc) = &self.u_view_proj {
                gl.uniform_matrix_4_f32_slice(Some(loc), false, &view_proj.to_cols_array());
            }

            if let Some(loc) = &self.u_camera_pos {
                gl.uniform_3_f32(Some(loc), camera_pos.x, camera_pos.y, camera_pos.z);
            }

            if let Some(loc) = &self.u_step_size {
                gl.uniform_1_f32(Some(loc), step_size);
            }

            if let Some(loc) = &self.u_value_min {
                gl.uniform_1_f32(Some(loc), value_range[0]);
            }

            if let Some(loc) = &self.u_value_max {
                gl.uniform_1_f32(Some(loc), value_range[1]);
            }

            // Set volume rotation matrix
            if let Some(loc) = &self.u_volume_rotation {
                gl.uniform_matrix_4_f32_slice(Some(loc), false, &volume_rotation.to_cols_array());
            }

            // Set opacity
            if let Some(loc) = &self.u_opacity {
                gl.uniform_1_f32(Some(loc), opacity);
            }

            // Bind volume texture
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_3D, self.volume_texture);
            if let Some(loc) = &self.u_volume {
                gl.uniform_1_i32(Some(loc), 0);
            }

            // Bind occupancy texture
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_3D, self.occupancy_texture);
            if let Some(loc) = &self.u_occupancy {
                gl.uniform_1_i32(Some(loc), 1);
            }
            if let Some(loc) = &self.u_occupancy_size {
                gl.uniform_1_f32(Some(loc), OCCUPANCY_GRID_SIZE as f32);
            }

            // Draw cube (36 vertices)
            gl.draw_arrays(glow::TRIANGLES, 0, 36);

            // Clean up state
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_3D, None);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_3D, None);
            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.disable(glow::CULL_FACE);
            gl.disable(glow::BLEND);
        }
    }

    /// Render coordinate axes
    pub fn render_axes(
        &self,
        gl: &glow::Context,
        view_proj: &glam::Mat4,
        volume_rotation: &glam::Mat4,
    ) {
        unsafe {
            gl.disable(glow::DEPTH_TEST);
            gl.line_width(2.0);

            gl.use_program(Some(self.axes_program));
            gl.bind_vertex_array(Some(self.axes_vao));

            // Set uniforms
            if let Some(loc) = &self.axes_u_view_proj {
                gl.uniform_matrix_4_f32_slice(Some(loc), false, &view_proj.to_cols_array());
            }

            // Model matrix: translate to corner of volume and apply rotation
            // Position axes at (-0.5, -0.5, -0.5) corner so they don't obscure the volume
            let translation = glam::Mat4::from_translation(glam::Vec3::new(-0.5, -0.5, -0.5));
            let model = translation * *volume_rotation;

            if let Some(loc) = &self.axes_u_model {
                gl.uniform_matrix_4_f32_slice(Some(loc), false, &model.to_cols_array());
            }

            // Draw 6 vertices as 3 lines
            gl.draw_arrays(glow::LINES, 0, 6);

            gl.bind_vertex_array(None);
            gl.use_program(None);
        }
    }

    /// Clean up OpenGL resources
    pub fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vao);
            if let Some(tex) = self.volume_texture {
                gl.delete_texture(tex);
            }
            if let Some(tex) = self.occupancy_texture {
                gl.delete_texture(tex);
            }
            gl.delete_program(self.axes_program);
            gl.delete_vertex_array(self.axes_vao);
            gl.delete_buffer(self.axes_vbo);
        }
    }
}
