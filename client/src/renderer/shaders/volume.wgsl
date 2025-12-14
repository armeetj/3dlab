// Volume ray marching shader

struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad0: f32,
    volume_dims: vec3<f32>,
    step_size: f32,
    value_min: f32,
    value_max: f32,
    _pad1: vec2<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var volume_texture: texture_3d<f32>;
@group(0) @binding(2) var volume_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
}

// Cube vertices for bounding box (front and back faces)
var<private> CUBE_VERTICES: array<vec3<f32>, 36> = array<vec3<f32>, 36>(
    // Front face
    vec3(-0.5, -0.5,  0.5), vec3( 0.5, -0.5,  0.5), vec3( 0.5,  0.5,  0.5),
    vec3(-0.5, -0.5,  0.5), vec3( 0.5,  0.5,  0.5), vec3(-0.5,  0.5,  0.5),
    // Back face
    vec3( 0.5, -0.5, -0.5), vec3(-0.5, -0.5, -0.5), vec3(-0.5,  0.5, -0.5),
    vec3( 0.5, -0.5, -0.5), vec3(-0.5,  0.5, -0.5), vec3( 0.5,  0.5, -0.5),
    // Top face
    vec3(-0.5,  0.5,  0.5), vec3( 0.5,  0.5,  0.5), vec3( 0.5,  0.5, -0.5),
    vec3(-0.5,  0.5,  0.5), vec3( 0.5,  0.5, -0.5), vec3(-0.5,  0.5, -0.5),
    // Bottom face
    vec3(-0.5, -0.5, -0.5), vec3( 0.5, -0.5, -0.5), vec3( 0.5, -0.5,  0.5),
    vec3(-0.5, -0.5, -0.5), vec3( 0.5, -0.5,  0.5), vec3(-0.5, -0.5,  0.5),
    // Right face
    vec3( 0.5, -0.5,  0.5), vec3( 0.5, -0.5, -0.5), vec3( 0.5,  0.5, -0.5),
    vec3( 0.5, -0.5,  0.5), vec3( 0.5,  0.5, -0.5), vec3( 0.5,  0.5,  0.5),
    // Left face
    vec3(-0.5, -0.5, -0.5), vec3(-0.5, -0.5,  0.5), vec3(-0.5,  0.5,  0.5),
    vec3(-0.5, -0.5, -0.5), vec3(-0.5,  0.5,  0.5), vec3(-0.5,  0.5, -0.5),
);

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let pos = CUBE_VERTICES[vertex_index];
    out.world_pos = pos + vec3(0.5); // Shift to [0,1] range for texture coords
    out.clip_position = uniforms.view_proj * vec4(pos, 1.0);
    return out;
}

// Ray-box intersection (returns t_near, t_far)
fn intersect_box(ray_origin: vec3<f32>, ray_dir: vec3<f32>) -> vec2<f32> {
    let inv_dir = 1.0 / ray_dir;
    let t0 = (vec3(0.0) - ray_origin) * inv_dir;
    let t1 = (vec3(1.0) - ray_origin) * inv_dir;
    let t_min = min(t0, t1);
    let t_max = max(t0, t1);
    let t_near = max(max(t_min.x, t_min.y), t_min.z);
    let t_far = min(min(t_max.x, t_max.y), t_max.z);
    return vec2(max(t_near, 0.0), t_far);
}

// Simple grayscale transfer function
fn transfer_function(density: f32) -> vec4<f32> {
    // Normalize density to [0,1] based on value range
    let normalized = clamp((density - uniforms.value_min) / (uniforms.value_max - uniforms.value_min), 0.0, 1.0);

    // Simple opacity curve - higher values are more opaque
    let alpha = pow(normalized, 2.0) * 0.5;

    // Grayscale color
    let color = vec3(normalized);

    return vec4(color, alpha);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Ray direction from camera to this fragment's world position
    let ray_origin = uniforms.camera_pos + vec3(0.5); // Shift camera to [0,1] space
    let ray_dir = normalize(in.world_pos - ray_origin);

    // Find ray entry/exit points in volume
    let t = intersect_box(ray_origin, ray_dir);

    if (t.x > t.y) {
        discard;
    }

    // Accumulated color and opacity
    var color = vec4(0.0);

    // Start ray marching
    var t_current = t.x;
    let max_steps = 256;

    for (var i = 0; i < max_steps; i++) {
        if (t_current > t.y || color.a > 0.95) {
            break;
        }

        let sample_pos = ray_origin + ray_dir * t_current;

        // Sample volume texture
        let density = textureSample(volume_texture, volume_sampler, sample_pos).r;

        // Apply transfer function
        let sample_color = transfer_function(density);

        // Front-to-back compositing
        color.r += (1.0 - color.a) * sample_color.a * sample_color.r;
        color.g += (1.0 - color.a) * sample_color.a * sample_color.g;
        color.b += (1.0 - color.a) * sample_color.a * sample_color.b;
        color.a += (1.0 - color.a) * sample_color.a;

        t_current += uniforms.step_size;
    }

    // Background color for transparent areas
    let bg_color = vec3(0.1, 0.1, 0.1);
    color = vec4(color.rgb + (1.0 - color.a) * bg_color, 1.0);

    return color;
}
