#version 300 es
precision highp float;

uniform mat4 u_view_proj;
uniform mat4 u_model;

layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_color;

out vec3 v_color;

void main() {
    v_color = a_color;
    gl_Position = u_view_proj * u_model * vec4(a_position, 1.0);
}
