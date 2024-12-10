#version 330

layout (location = 0) in vec2 texcoord;
//layout (location = 1) in uint color;
//layout (location = 2) in uint meta;

uniform float left;
uniform float right;
uniform float keyboard_height;
uniform vec2 resolution;

uniform uint color;
uniform uint meta;

out vec3 k_color;
out vec2 v_texcoord;
out float k_pressed;
out float k_black;

void main() {
    float left = left * 2.0 - 1.0;
    float right = right * 2.0 - 1.0;

    uint clr = color;
    vec3 color = vec3(uvec3((clr & uint(0xFF)), (clr >> 8) & uint(0xFF), (clr >> 16) & uint(0xFF))) / 256.0;

    uint meta = meta;

    bool pressed = (meta & uint(2)) == uint(2);
    bool black = (meta & uint(1)) == uint(1);

    k_pressed = pressed ? 1.0 : 0.0;
    k_black = black ? 1.0 : 0.0;
    v_texcoord = texcoord;

    if (int(gl_VertexID % 4) == 0) {
        gl_Position = vec4(left, -1.0, 0.0, 1.0);
    } else if (int(gl_VertexID % 4) == 1) {
        gl_Position = vec4(right, -1.0, 0.0, 1.0);
    } else if (int(gl_VertexID % 4) == 2) {
        gl_Position = vec4(right, keyboard_height * 2.0 - 1.0, 0.0, 1.0);
    } else {
        gl_Position = vec4(left, keyboard_height * 2.0 - 1.0, 0.0, 1.0);
    }

    if (black && (gl_VertexID == 0 || gl_VertexID == 1)) {
        gl_Position.y += keyboard_height * 2.0/3.0;
    }

    if (pressed) {
        k_color = color;
        if ((gl_VertexID == 2 || gl_VertexID == 3) && !black) {
            k_color *= 0.6;
        } else if ((gl_VertexID == 0 || gl_VertexID == 1) && black) {
            k_color *= 0.3;
        }
    }
    else k_color = (black ? vec3(0.0) : vec3(1.0));
}