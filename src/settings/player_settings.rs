use std::path::absolute;
use super::config::get_config;

pub struct PlayerSettings {
    pub show_ui: bool,
    pub tick_based: bool,
    pub fullscreen: bool
}

impl PlayerSettings {
    pub fn new() -> Self {
        Self {
            show_ui: true,
            tick_based: true,
            fullscreen: false
        }
    }
    
    pub fn load_settings(&mut self) {
        let mut config = get_config();
        if !config.sections().contains(&String::from("player")) {
            config.set("player", "show_ui", Some(true.to_string()));
            config.set("player", "tick_based", Some(true.to_string()));
        } else {
            self.show_ui = config.getbool("player", "show_ui").unwrap().unwrap_or(true);
            self.tick_based = config.getbool("player", "tick_based").unwrap().unwrap_or(true);
        }
    }

    pub fn save_settings(&mut self) {
        let mut config = get_config();
        config.set("player", "show_ui", Some(self.show_ui.to_string()));
        config.set("player", "tick_based", Some(self.tick_based.to_string()));
        config.write(absolute("./config.ini").unwrap()).unwrap();
    }
}