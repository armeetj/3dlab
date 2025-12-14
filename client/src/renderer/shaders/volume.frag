#version 300 es
precision highp float;
precision highp sampler3D;

uniform vec3 u_camera_pos;
uniform float u_step_size;
uniform float u_value_min;
uniform float u_value_max;
uniform sampler3D u_volume;
uniform mat4 u_volume_rotation;

in vec3 v_world_pos;
out vec4 out_color;

// Ray-box intersection (returns t_near, t_far)
vec2 intersect_box(vec3 ray_origin, vec3 ray_dir) {
    vec3 inv_dir = 1.0 / ray_dir;
    vec3 t0 = (vec3(0.0) - ray_origin) * inv_dir;
    vec3 t1 = (vec3(1.0) - ray_origin) * inv_dir;
    vec3 t_min = min(t0, t1);
    vec3 t_max = max(t0, t1);
    float t_near = max(max(t_min.x, t_min.y), t_min.z);
    float t_far = min(min(t_max.x, t_max.y), t_max.z);
    return vec2(max(t_near, 0.0), t_far);
}

// Simple grayscale transfer function
vec4 transfer_function(float density) {
    // Normalize density to [0,1] based on value range
    float normalized = clamp((density - u_value_min) / (u_value_max - u_value_min), 0.0, 1.0);

    // Simple opacity curve - higher values are more opaque
    float alpha = pow(normalized, 2.0) * 0.5;

    // Grayscale color
    vec3 color = vec3(normalized);

    return vec4(color, alpha);
}

void main() {
    // Ray direction from camera to this fragment's world position
    vec3 ray_origin = u_camera_pos + vec3(0.5); // Shift camera to [0,1] space
    vec3 ray_dir = normalize(v_world_pos - ray_origin);

    // Find ray entry/exit points in volume
    vec2 t = intersect_box(ray_origin, ray_dir);

    if (t.x > t.y) {
        discard;
    }

    // Accumulated color and opacity
    vec4 color = vec4(0.0);

    // Start ray marching
    float t_current = t.x;
    const int max_steps = 128;

    for (int i = 0; i < max_steps; i++) {
        if (t_current > t.y || color.a > 0.95) {
            break;
        }

        vec3 sample_pos = ray_origin + ray_dir * t_current;

        // Apply inverse rotation (transpose for orthogonal matrix) around volume center
        vec3 centered = sample_pos - vec3(0.5);
        vec3 rotated = (transpose(u_volume_rotation) * vec4(centered, 1.0)).xyz;
        vec3 rotated_pos = rotated + vec3(0.5);

        // Sample volume texture
        // Texture axes are swapped: (Z, Y, X) due to row-major memory layout
        vec3 tex_coord = vec3(rotated_pos.z, rotated_pos.y, rotated_pos.x);

        // Skip samples outside the unit cube (after rotation, some rays may exit early)
        if (any(lessThan(tex_coord, vec3(0.0))) || any(greaterThan(tex_coord, vec3(1.0)))) {
            t_current += u_step_size;
            continue;
        }

        float density = texture(u_volume, tex_coord).r;

        // Empty space skipping: take larger steps through low-density regions
        float normalized_density = (density - u_value_min) / (u_value_max - u_value_min);
        if (normalized_density < 0.02) {
            t_current += u_step_size * 4.0;
            continue;
        }

        // Apply transfer function
        vec4 sample_color = transfer_function(density);

        // Front-to-back compositing
        color.rgb += (1.0 - color.a) * sample_color.a * sample_color.rgb;
        color.a += (1.0 - color.a) * sample_color.a;

        t_current += u_step_size;
    }

    // Background color for transparent areas
    vec3 bg_color = vec3(0.1, 0.1, 0.1);
    out_color = vec4(color.rgb + (1.0 - color.a) * bg_color, 1.0);
}
