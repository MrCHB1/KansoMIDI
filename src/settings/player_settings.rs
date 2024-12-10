use configparser::ini::Ini;
use std::fs::File;
use std::ops::Index;
use std::path::absolute;
use super::config::get_config;

pub struct PlayerSettings {
    pub show_ui: bool
}

impl PlayerSettings {
    pub fn new() -> Self {
        Self {
            show_ui: true
        }
    }
    
    pub fn load_settings(&mut self) {
        let mut config = get_config();
        if !config.sections().contains(&String::from("player")) {
            config.set("player", "show_ui", Some(true.to_string()));
        } else {
            self.show_ui = config.getbool("player", "show_ui").unwrap().unwrap();
        }
    }

    pub fn save_settings(&mut self) {
        let mut config = get_config();
        config.set("player", "show_ui", Some(self.show_ui.to_string()));
        config.write(absolute("./config.ini").unwrap()).unwrap();
    }
}