mod midi;
mod util;
mod rendering;
mod settings;
mod audio;

use rendering::window::MainWindow;
use util::global_timer::GlobalTimer;
use std::sync::{Arc, Mutex};

fn main() {
    // play state
    let glob_timer: Arc<Mutex<GlobalTimer>> = Arc::new(Mutex::new(GlobalTimer::new()));
    glob_timer.lock().unwrap().pause();

    let _ = MainWindow::new(1280, 720, "KansoMIDI", glob_timer.clone());
}