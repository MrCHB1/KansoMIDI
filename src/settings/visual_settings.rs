use crate::util::color_funcs::*;
use crate::settings::config::*;

pub struct VisualSettings {
    pub bar_color: [f32; 3],
    pub background_color: [f32; 3],
    pub note_size: f32,
    pub palette_index: usize,
    pub keyboard_range_id: usize,
    pub kb_first_key: usize,
    pub kb_last_key: usize
}

impl VisualSettings {
    pub fn new() -> Self {
        Self {
            bar_color: [1.0, 0.0, 0.0],
            background_color: [0.5, 0.5, 0.5],
            note_size: 0.25,
            palette_index: 0,
            keyboard_range_id: 0, /* 0: 88 keys, 1: 128 keys, 2: 256 keys, 3: MIDI key range, 4: Custom */
            kb_first_key: 21,
            kb_last_key: 108
        }
    }

    pub fn set_kb_first_key(&mut self, mut first_key: i32) -> () {
        if first_key > self.kb_last_key as i32 {
            first_key = self.kb_last_key as i32
        }

        if first_key > 256 {
            first_key = 256;
        }
        if first_key < 0 {
            first_key = 0;
        }

        self.kb_first_key = first_key as usize;
    }

    pub fn set_kb_last_key(&mut self, mut last_key: i32) -> () {
        if last_key < self.kb_first_key as i32 {
            last_key = self.kb_first_key as i32
        }

        if last_key > 256 {
            last_key = 256;
        }
        if last_key < 0 {
            last_key = 0;
        }

        self.kb_last_key = last_key as usize;
    }

    pub fn load_settings(&mut self) {
        let mut config = get_config();
        if !config.sections().contains(&String::from("visual")) {
            config.set("visual", "bar_color", Some(encode_rgb(self.bar_color).to_string()));
            config.set("visual", "background_color", Some(encode_rgb(self.background_color).to_string()));
            config.set("visual", "note_size", Some(self.note_size.to_string()));
            config.set("visual", "palette_index", Some(self.palette_index.to_string()));
            config.set("visual", "keyboard_range_id", Some(self.keyboard_range_id.to_string()));
            config.set("visual", "kb_first_key", Some(self.kb_first_key.to_string()));
            config.set("visual", "kb_last_key", Some(self.kb_last_key.to_string()));

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
            self.keyboard_range_id = config.getuint("visual", "keyboard_range_id").unwrap()
                .unwrap_or(0) as usize;
            self.kb_first_key = config.getuint("visual", "kb_first_key").unwrap()
                .unwrap_or(21) as usize;
            self.kb_last_key = config.getuint("visual", "kb_last_key").unwrap()
                .unwrap_or(108) as usize;
        }
    }

    // sometimes the user can mess with the settings and set the values to unethical ones
    // for example, manually setting the first key to be 136 and the last key to be 3.
    // this function ensures that these values are valid
    /*fn validate_settings(&mut self) -> () {
        /* dont need to validate the bar/background color */
        if self.note_size < 0.01 {
            self.note_size = 0.01;
        }
        if self.note_size > 4.0 {
            self.note_size = 4.0;
        }

        if self.keyboard_range_id > 4 {
            self.keyboard_range_id = 4;
        }
        
        if self.kb_first_key < 0 {
            self.kb_first_key = 0;
        }
        if self.kb_first_key > self.kb_last_key {
            self.kb_first_key = self.kb_last_key;
        }

        if self.kb_last_key > 256 {
            self.kb_last_key = 256;
        }
        if self.kb_last_key < self.kb_first_key {
            self.kb_last_key = self.kb_first_key;
        }
    }*/

    pub fn save_settings(&mut self) {
        let mut config = get_config();
        config.set("visual", "bar_color", Some(encode_rgb(self.bar_color).to_string()));
        config.set("visual", "background_color", Some(encode_rgb(self.background_color).to_string()));
        config.set("visual", "note_size", Some(self.note_size.to_string()));
        config.set("visual", "palette_index", Some(self.palette_index.to_string()));
        config.set("visual", "keyboard_range_id", Some(self.keyboard_range_id.to_string()));
        config.set("visual", "kb_first_key", Some(self.kb_first_key.to_string()));
        config.set("visual", "kb_last_key", Some(self.kb_last_key.to_string()));
        config.write(std::path::absolute("./config.ini").unwrap()).unwrap();
    }
}