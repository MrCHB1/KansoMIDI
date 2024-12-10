#version 330

out vec4 key_color;

in vec3 k_color;
in vec2 v_texcoord;
in float k_pressed;
in float k_black;

void main() {
    key_color = vec4(k_color, 1.0);
    if (k_black < 0.5) {
        if (v_texcoord.y < 0.05 && k_pressed < 0.5) {
            key_color *= 0.5;
        }
        if (v_texcoord.x < 0.05 || v_texcoord.x > 0.95) {
            key_color *= 0.3;
        }
    } else {
        if (v_texcoord.y < 0.1 || v_texcoord.x < 0.07 || v_texcoord.x > 0.93) {
            key_color += 0.2;
        }
    }
    
}