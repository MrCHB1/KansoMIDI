use configparser::ini::Ini;
use std::fs::File;
use std::path::absolute;

pub fn get_config() -> Ini {
    let mut config = Ini::new();
    let path = absolute("./config.ini").unwrap();
    if !path.exists() {
        File::create(absolute("./config.ini").unwrap()).unwrap();
    }
    config.load(absolute("./config.ini").unwrap()).unwrap();
    config
}