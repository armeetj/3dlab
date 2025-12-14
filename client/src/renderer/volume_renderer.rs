use glow::HasContext;

use super::camera::Camera;

const VERTEX_SHADER: &str = include_str!("shaders/volume.vert");
const FRAGMENT_SHADER: &str = include_str!("shaders/volume.frag");

/// Volume renderer using OpenGL ray marching
pub struct VolumeRenderer {
    program: glow::Program,
    vao: glow::VertexArray,
    volume_texture: Option<glow::Texture>,
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

            // Create VAO (required for WebGL2/OpenGL ES 3.0)
            let vao = gl.create_vertex_array().unwrap();

            Self {
                program,
                vao,
                volume_texture: None,
                camera: Camera::default(),
                volume_dims: [1, 1, 1],
                value_range: [0.0, 1.0],
                u_view_proj,
                u_camera_pos,
                u_step_size,
                u_value_min,
                u_value_max,
                u_volume,
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
            // Delete old texture if exists
            if let Some(tex) = self.volume_texture.take() {
                gl.delete_texture(tex);
            }

            // Create 3D texture
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
            gl.tex_image_3d(
                glow::TEXTURE_3D,
                0,
                glow::R32F as i32,
                dims[0] as i32,
                dims[1] as i32,
                dims[2] as i32,
                0,
                glow::RED,
                glow::FLOAT,
                Some(bytes),
            );

            gl.bind_texture(glow::TEXTURE_3D, None);

            self.volume_texture = Some(texture);
        }
    }

    /// Check if volume data is loaded
    pub fn has_volume(&self) -> bool {
        self.volume_texture.is_some()
    }

    /// Render the volume using internal camera
    pub fn render(&self, gl: &glow::Context, aspect_ratio: f32) {
        let view_proj = self.camera.view_projection_matrix(aspect_ratio);
        let camera_pos = self.camera.position();
        self.render_with_params(gl, &view_proj, &camera_pos, 0.005, self.value_range);
    }

    /// Render the volume with explicit parameters
    pub fn render_with_params(
        &self,
        gl: &glow::Context,
        view_proj: &glam::Mat4,
        camera_pos: &glam::Vec3,
        step_size: f32,
        value_range: [f32; 2],
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

            // Bind volume texture
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_3D, self.volume_texture);
            if let Some(loc) = &self.u_volume {
                gl.uniform_1_i32(Some(loc), 0);
            }

            // Draw cube (36 vertices)
            gl.draw_arrays(glow::TRIANGLES, 0, 36);

            // Clean up state
            gl.bind_texture(glow::TEXTURE_3D, None);
            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.disable(glow::CULL_FACE);
            gl.disable(glow::BLEND);
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
        }
    }
}
