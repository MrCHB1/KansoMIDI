use std::time::Instant;

pub struct GlobalTimer {
    pub time: Instant,
    pub midi_time: f32,
    pub paused: bool,
    pub speed: f32,

    pub time_changed: bool,
    pub pause_changed: bool,
    pub speed_changed: bool,
}

impl GlobalTimer {
    pub fn new() -> Self {
        Self {
            time: Instant::now(),
            midi_time: 0.0,
            paused: true,
            speed: 1.0,

            time_changed: false,
            pause_changed: false,
            speed_changed: false
        }
    }

    pub fn pause(&mut self) -> () {
        if self.paused { return; }
        self.midi_time += self.time.elapsed().as_secs_f32() * self.speed;
        let pause = self.paused;
        self.paused = true;
        self.time_changed = true;
        if !pause { self.pause_changed = true; }
    }

    pub fn play(&mut self) -> () {
        self.time = Instant::now();
        let pause = self.paused;
        self.paused = false;
        if pause { self.pause_changed = true; }
    }

    pub fn reset(&mut self) -> () {
        self.midi_time = 0.0;
        let pause = self.paused;
        self.paused = true;
        self.time_changed = true;
        if !pause { self.pause_changed = true; }
    }

    pub fn get_time(&mut self) -> f32 {
        if self.paused { return self.midi_time; }
        return self.midi_time + self.time.elapsed().as_secs_f32() * self.speed;
    }

    pub fn navigate(&mut self, time: f32) -> () {
        self.time = Instant::now();
        self.midi_time = time;
        self.time_changed = true;
    }

    pub fn change_speed(&mut self, speed: f32) -> () {
        self.midi_time = self.get_time();
        self.time = Instant::now();
        self.speed = speed;
        self.speed_changed = true;
    }
}