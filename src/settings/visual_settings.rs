use configparser::ini::Ini;
use std::fs::File;
use crate::util::color_funcs::*;
use crate::settings::config::*;

pub struct VisualSettings {
    pub bar_color: [f32; 3],
    pub background_color: [f32; 3],
    pub note_size: f32,
    pub palette_index: usize
}

impl VisualSettings {
    pub fn new() -> Self {
        Self {
            bar_color: [1.0, 0.0, 0.0],
            background_color: [0.5, 0.5, 0.5],
            note_size: 0.25,
            palette_index: 0,
        }
    }

    pub fn load_settings(&mut self) {
        let mut config = get_config();
        if !config.sections().contains(&String::from("visual")) {
            config.set("visual", "bar_color", Some(encode_rgb(self.bar_color).to_string()));
            config.set("visual", "background_color", Some(encode_rgb(self.background_color).to_string()));
            config.set("visual", "note_size", Some(self.note_size.to_string()));
            config.set("visual", "palette_index", Some(self.palette_index.to_string()));
            println!("No visual settings found, default values loaded.");
        } else {
            self.bar_color = decode_rgb(config.getuint("visual", "bar_color").unwrap()
                .unwrap_or(0xFF0000) as u32);
            self.background_color = decode_rgb(config.getuint("visual", "background_color").unwrap()
                .unwrap_or(0xAAAAAA) as u32);
            self.note_size = config.getfloat("visual", "note_size").unwrap()
                .unwrap_or(0.25) as f32;
            self.palette_index = config.getuint("visual", "palette_index").unwrap()
                .unwrap_or(0) as usize;
        }
    }

    pub fn save_settings(&mut self) {
        let mut config = get_config();
        config.set("visual", "bar_color", Some(encode_rgb(self.bar_color).to_string()));
        config.set("visual", "background_color", Some(encode_rgb(self.background_color).to_string()));
        config.set("visual", "note_size", Some(self.note_size.to_string()));
        config.set("visual", "palette_index", Some(self.palette_index.to_string()));
        config.write(std::path::absolute("./config.ini").unwrap()).unwrap();
    }
}