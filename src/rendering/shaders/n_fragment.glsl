#version 330

out vec4 note_color;

in vec3 n_color;
in vec2 v_texcoord;

uniform vec2 resolution;

in vec2 note_size;

void main() {
    vec3 col = n_color;
    vec2 texel_size = vec2(1.0 / resolution.x, 1.0 / resolution.y);
    vec2 mul = vec2(resolution.x / note_size.x,
                    resolution.y / note_size.y);
    note_color = vec4(col, 1.0);
}