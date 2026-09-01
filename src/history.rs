use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageHistory {
    pub launches: HashMap<String, u32>,
}

impl UsageHistory {
    pub fn file_path() -> PathBuf {
        crate::config::get_app_storage_dir().join("history.json")
    }

    pub fn load() -> Self {
        let path = Self::file_path();
        if let Ok(file) = File::open(path) {
            let reader = BufReader::new(file);
            serde_json::from_reader(reader).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn record_launch(&mut self, app_id: &str) {
        *self.launches.entry(app_id.to_string()).or_insert(0) += 1;
        self.save();
    }

    pub fn get_launch_count(&self, app_id: &str) -> u32 {
        *self.launches.get(app_id).unwrap_or(&0)
    }

    pub fn save(&self) {
        let path = Self::file_path();
        if let Ok(file) = File::create(path) {
            let writer = BufWriter::new(file);
            let _ = serde_json::to_writer(writer, self);
        }
    }
}
