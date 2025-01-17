use std::path::absolute;
use std::ops::Index;

use super::config::*;

pub struct AudioSettings {
    pub layer_count: i32,
    pub soundfont_paths: Vec<String>,
    pub active_soundfonts: Vec<bool>,
    pub audio_fps: f32,

    pub limiter_attack: f32,
    pub limiter_release: f32,

    pub misc_transpose: i32,
}

impl AudioSettings {
    pub fn new() -> Self {
        Self {
            layer_count: 5,
            soundfont_paths: Vec::new(),
            active_soundfonts: Vec::new(),
            audio_fps: 0.0,

            limiter_attack: 0.01,
            limiter_release: 1.0,

            misc_transpose: 0
        }
    }

    pub fn load_settings(&mut self) {
        let mut config = get_config();
        if !config.sections().contains(&String::from("audio")) {
            config.set("audio", "layer_count", Some(self.layer_count.to_string()));
            for i in 0..self.soundfont_paths.len() {
                config.set("audio", 
                    format!("soundfont_paths_{}", i).as_str(),
                    Some(self.soundfont_paths.index(i).to_string()));
            }
            config.set("audio", "audio_fps", Some(self.audio_fps.to_string()));
            config.set("audio", "limiter_attack", Some(self.limiter_attack.to_string()));
            config.set("audio", "limiter_release", Some(self.limiter_release.to_string()));
            println!("No audio settings found, default values loaded.");
        } else {
            self.layer_count = config.getint("audio", "layer_count").unwrap()
                .unwrap_or(5) as i32;
            let mut i = 0;
            loop {
                if let Some(sf) = 
                    config.get("audio", format!("soundfont_paths_{}", i).as_str()) {
                        self.soundfont_paths.push(sf);
                    }
                else {
                    break;
                }
                i += 1;
            }
            self.audio_fps = config.getfloat("audio", "audio_fps").unwrap()
                .unwrap_or(0.0) as f32;
            self.limiter_attack = config.getfloat("audio", "limiter_attack").unwrap()
                .unwrap_or(0.01) as f32;
            self.limiter_release = config.getfloat("audio", "limiter_release").unwrap()
                .unwrap_or(1.0) as f32;
        }
    }

    pub fn save_settings(&mut self) {
        let mut config = get_config();
        config.set("audio", "layer_count", Some(self.layer_count.to_string()));
        for i in 0..self.soundfont_paths.len() {
            config.set("audio", 
                format!("soundfont_paths_{}", i).as_str(),
                Some(self.soundfont_paths.index(i).to_string()));
        }
        config.set("audio", "audio_fps", Some(self.audio_fps.to_string()));
        config.set("audio", "limiter_attack", Some(self.limiter_attack.to_string()));
        config.set("audio", "limiter_release", Some(self.limiter_release.to_string()));
        config.write(absolute("./config.ini").unwrap()).unwrap();
    }
}