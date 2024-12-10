#version 330

in vec3 b_color;
in vec2 b_texcoord;

out vec4 bar_color;

void main() {
    bar_color = vec4(b_color * b_texcoord.y, 1.0);
}