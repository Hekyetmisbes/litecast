use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::PathBuf;

pub fn get_log_dir() -> PathBuf {
    let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    let log_dir = PathBuf::from(local_app_data).join("LiteCast").join("logs");
    let _ = create_dir_all(&log_dir);
    log_dir
}

pub fn log_info(msg: &str) {
    let log_file = get_log_dir().join("litecast.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_file) {
        let _ = writeln!(file, "[INFO] {}", msg);
    }
}

pub fn log_error(msg: &str) {
    let log_file = get_log_dir().join("litecast.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_file) {
        let _ = writeln!(file, "[ERROR] {}", msg);
    }
}
