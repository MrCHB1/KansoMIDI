use std::process::Command;

#[cfg(target_os = "macos")]
pub fn open_directory_in_explorer(directory: &str) {
    Command::new("open")
        .arg(directory)
        .spawn()
        .unwrap();
}

#[cfg(target_os = "windows")]
pub fn open_directory_in_explorer(directory: &str) {
    Command::new("explorer")
        .arg(directory)
        .spawn()
        .unwrap();
}

#[cfg(target_os = "linux")]
pub fn open_directory_in_explorer(directory: &str) {
    Command::new("xdg-open")
        .arg(directory)
        .spawn()
        .unwrap();
}