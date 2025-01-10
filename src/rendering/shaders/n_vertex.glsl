#version 330

layout (location = 0) in vec2 texcoord;
layout (location = 1) in vec2 note_times;
layout (location = 2) in uint colors;

uniform float left;
uniform float right;
uniform float keyboard_height;
uniform vec2 resolution;

out vec3 n_color;
out vec2 v_texcoord;

out vec2 note_size;

void main() {
    float left = left * 2.0 - 1.0;
    float right = right * 2.0 - 1.0;
    float start = note_times.x * 2.0 - 1.0;
    float end = note_times.y * 2.0 - 1.0;
    uint clr = colors;
    vec3 color = vec3(
        uvec3((clr & uint(0xFF)),
              (clr >> 8) & uint(0xFF),
              (clr >> 16) & uint(0xFF)
        )
    ) / 256.0;
    vec3 bdr = color * 0.3;
    v_texcoord = texcoord;
    note_size = vec2(abs((right * 0.5 + 0.5) - (left * 0.5 + 0.5)), abs((end * 0.5 + 0.5) - (start * 0.5 + 0.5)));

    // 0 1 3 1 2 3
    /*if (gl_VertexID % 6 == 0) {
        gl_Position = vec4(
            left,
            start,
            0.0, 1.0);
        n_color = color;
    } else if (gl_VertexID % 6 == 1 || gl_VertexID % 6 == 3) {
        gl_Position = vec4(
            right,
            start,
            0.0, 1.0);
        n_color = color * 0.9;
    } else if (gl_VertexID % 6 == 4) {
        gl_Position = vec4(
            right,
            end,
            0.0, 1.0);
        n_color = color * 0.9;
    } else {
        gl_Position = vec4(
            left,
            end,
            0.0, 1.0);
        n_color = color;
    }*/

    int n_type = gl_VertexID / 4;
    if (n_type == 0 || (n_type == 1 && note_size.y > 6.0 / resolution.y)) {
        if (int(gl_VertexID % 4) == 0) {
            gl_Position = vec4(left, start, 0.0, 1.0);
            n_color = color + 0.2;
            if (n_type == 1) {
                gl_Position.xy += vec2(3.0 / resolution.x, 3.0 / resolution.y);
            }
        } else if (int(gl_VertexID % 4) == 1) {
            gl_Position = vec4(right, start, 0.0, 1.0);
            n_color = color;
            if (n_type == 1) {
                gl_Position.xy += vec2(-3.0 / resolution.x, 3.0 / resolution.y);
            }
        } else if (int(gl_VertexID % 4) == 2) {
            gl_Position = vec4(right, end, 0.0, 1.0);
            n_color = color;
            if (n_type == 1) {
                gl_Position.xy += vec2(-3.0 / resolution.x, -3.0 / resolution.y);
            }
        } else if (int(gl_VertexID % 4) == 3 ) {
            gl_Position = vec4(left, end, 0.0, 1.0);
            n_color = color + 0.2;
            if (n_type == 1) {
                gl_Position.xy += vec2(3.0 / resolution.x, -3.0 / resolution.y);
            }
        }
    }
    if (n_type == 0) {
        n_color = bdr;
    }
    gl_Position.y = gl_Position.y * 0.5 + 0.5;
    gl_Position.y = gl_Position.y * (1.0 - keyboard_height) + keyboard_height;
    gl_Position.y = gl_Position.y * 2.0 - 1.0;

    //gl_Position = vec4(left + gl_VertexID % 2, (gl_VertexID / 2) % 2, 0.0, 1.0);
}