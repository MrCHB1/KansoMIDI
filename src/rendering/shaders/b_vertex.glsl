#version 330

layout (location = 0) in vec2 texcoord;

uniform float keyboard_height;
uniform vec3 color;

out vec3 b_color;
out vec2 b_texcoord;

void main() {
    vec2 pos;
    if (gl_VertexID % 4 == 0) {
       pos = vec2(0.0, keyboard_height);
    } else if (gl_VertexID % 4 == 1) {
        pos = vec2(1.0, keyboard_height);
    } else if (gl_VertexID % 4 == 2) {
        pos = vec2(1.0, keyboard_height * 1.05);
    } else {
        pos = vec2(0.0, keyboard_height * 1.05);
    }
    b_color = color;
    b_texcoord = texcoord;
    gl_Position = vec4(pos * 2.0 - 1.0, 0.0, 1.0);
}