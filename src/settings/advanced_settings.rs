use std::path::absolute;

use super::config::*;
extern crate num_cpus;

pub struct AdvancedSettings {
    pub limit_fps: bool,
    pub max_fps: usize,
    pub per_key_thread_count: usize,
    pub per_chan_thread_count: usize
}

impl AdvancedSettings {
    pub fn new() -> Self {
        Self {
            limit_fps: true,
            max_fps: 60,
            per_key_thread_count: 1,
            per_chan_thread_count: 0,
        }
    }

    pub fn set_max_fps(&mut self, mut max_fps: i32) -> () {
        if max_fps < 15 {
            max_fps = 15;
        }
        self.max_fps = max_fps as usize;
    }

    pub fn set_per_key_thread_count(&mut self, mut per_key_threads: i32) -> () {
        if per_key_threads < 0 {
            per_key_threads = 0;
        }
        let num_cores = num_cpus::get() as i32;
        if per_key_threads >= num_cores {
            per_key_threads = num_cores;
        }
        self.per_key_thread_count = per_key_threads as usize;
    }

    pub fn set_per_chan_thread_count(&mut self, mut per_chan_threads: i32) -> () {
        if per_chan_threads < 0 {
            per_chan_threads = 0;
        }
        let num_cores = num_cpus::get() as i32;
        if per_chan_threads >= num_cores {
            per_chan_threads = num_cores;
        }
        self.per_chan_thread_count = per_chan_threads as usize;
    }
    
    pub fn load_settings(&mut self) {
        let mut config = get_config();
        if !config.sections().contains(&String::from("advanced")) {
            config.set("advanced", "limit_fps", Some(self.limit_fps.to_string()));
            config.set("advanced", "max_fps", Some(self.max_fps.to_string()));
            config.set("advanced", "per_key_thread_count", Some(self.per_key_thread_count.to_string()));
            config.set("advanced", "per_chan_thread_count", Some(self.per_chan_thread_count.to_string()));
        } else {
            //self.show_ui = config.getbool("player", "show_ui").unwrap().unwrap();
            self.limit_fps = config.getbool("advanced", "limit_fps").unwrap()
                .unwrap_or(true);
            self.max_fps = config.getuint("advanced", "max_fps").unwrap()
                .unwrap_or(60) as usize;
            self.per_key_thread_count = config.getuint("advanced", "per_key_thread_count").unwrap()
                .unwrap_or(1) as usize;
            self.per_chan_thread_count = config.getuint("advanced", "per_chan_thread_count").unwrap()
                .unwrap_or(0) as usize;
        }
    }

    pub fn save_settings(&mut self) {
        let mut config = get_config();
        //config.set("player", "show_ui", Some(self.show_ui.to_string()));
        config.set("advanced", "limit_fps", Some(self.limit_fps.to_string()));
        config.set("advanced", "max_fps", Some(self.max_fps.to_string()));
        config.set("advanced", "per_key_thread_count", Some(self.per_key_thread_count.to_string()));
        config.set("advanced", "per_chan_thread_count", Some(self.per_chan_thread_count.to_string()));
        config.write(absolute("./config.ini").unwrap()).unwrap();
    }
}