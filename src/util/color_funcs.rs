pub fn decode_rgb(rgb: u32) -> [f32; 3] {
    let r = (rgb & 0xFF0000) >> 16;
    let g = (rgb & 0xFF00) >> 8;
    let b = rgb & 0xFF;
    return [(r as f32) / 255.0, (g as f32) / 255.0, (b as f32) / 255.0];
}

pub fn encode_rgb(rgb: [f32; 3]) -> u32 {
    return 
        (((rgb[0] * 255.0) as u32) << 16) |
        (((rgb[1] * 255.0) as u32) << 8) |
        ((rgb[2] * 255.0) as u32);
}

pub fn rgb_to_hsv(rgb: [f32; 3]) -> [f32; 3] {
    let r = rgb[0];
    let g = rgb[1];
    let b = rgb[2];
    let cmax = r.max(g).max(b);
    let cmin = r.min(g).min(b);

    let delta = cmax - cmin;

    let hue = if delta == 0.0 {
        0.0f32
    } else if cmax == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if cmax == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else if cmax == b {
        60.0 * (((r - g) / delta) + 4.0)
    } else {
        0.0f32
    };

    let sat = if cmax == 0.0f32 {
        0.0f32
    } else {
        delta / cmax
    };

    let val = cmax;

    return [hue, sat, val];
}

pub fn hsv_to_rgb(hsv: [f32; 3]) -> [f32; 3] {
    let hue = hsv[0];
    let val = hsv[1];
    let sat = hsv[2];

    let c = val * sat;
    let x = c * (1.0 - (((hue / 60.0) % 2.0) - 1.0).abs());
    let m = val - c;
    let (r, g, b): (f32, f32, f32) = if hue >= 0.0 && hue < 60.0 {
        (c, x, 0.0f32)
    } else if hue >= 60.0 && hue < 120.0 {
        (x, c, 0.0f32)
    } else if hue >= 120.0 && hue < 180.0 {
        (0.0f32, c, x)
    } else if hue >= 180.0 && hue < 240.0 {
        (0.0f32, x, c)
    } else if hue >= 240.0 && hue < 300.0 {
        (x, 0.0f32, c)
    } else {
        (c, 0.0f32, x)
    };

    return [r+m, g+m, b+m];
}