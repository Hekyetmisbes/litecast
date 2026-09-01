use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, create_dir_all};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use windows::Win32::Foundation::COLORREF;

pub const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeColors {
    pub name: &'static str,
    pub primary: COLORREF,
    pub primary_hover: COLORREF,
    pub primary_light: COLORREF,
    pub primary_border: COLORREF,
    pub text_on_primary: COLORREF,
    pub preview_dot: COLORREF,
}

pub const THEME_PRESETS: [ThemeColors; 7] = [
    ThemeColors {
        name: "Raycast Turuncu",
        primary: rgb(255, 99, 50),
        primary_hover: rgb(235, 80, 35),
        primary_light: rgb(255, 160, 125),
        primary_border: rgb(255, 125, 85),
        text_on_primary: rgb(255, 255, 255),
        preview_dot: rgb(255, 99, 50),
    },
    ThemeColors {
        name: "Elektrik Mavisi",
        primary: rgb(37, 99, 235),
        primary_hover: rgb(59, 130, 246),
        primary_light: rgb(147, 197, 253),
        primary_border: rgb(96, 165, 250),
        text_on_primary: rgb(255, 255, 255),
        preview_dot: rgb(37, 99, 235),
    },
    ThemeColors {
        name: "Zümrüt Yeşili",
        primary: rgb(16, 185, 129),
        primary_hover: rgb(5, 150, 105),
        primary_light: rgb(110, 231, 183),
        primary_border: rgb(52, 211, 153),
        text_on_primary: rgb(255, 255, 255),
        preview_dot: rgb(16, 185, 129),
    },
    ThemeColors {
        name: "Siber Mor",
        primary: rgb(139, 92, 246),
        primary_hover: rgb(124, 58, 237),
        primary_light: rgb(196, 181, 253),
        primary_border: rgb(167, 139, 250),
        text_on_primary: rgb(255, 255, 255),
        preview_dot: rgb(139, 92, 246),
    },
    ThemeColors {
        name: "Gül Kırmızı",
        primary: rgb(244, 63, 94),
        primary_hover: rgb(225, 29, 72),
        primary_light: rgb(253, 164, 175),
        primary_border: rgb(251, 113, 133),
        text_on_primary: rgb(255, 255, 255),
        preview_dot: rgb(244, 63, 94),
    },
    ThemeColors {
        name: "Kehribar Altın",
        primary: rgb(245, 158, 11),
        primary_hover: rgb(217, 119, 6),
        primary_light: rgb(252, 211, 77),
        primary_border: rgb(251, 191, 36),
        text_on_primary: rgb(255, 255, 255),
        preview_dot: rgb(245, 158, 11),
    },
    ThemeColors {
        name: "Krom Gümüş",
        primary: rgb(228, 228, 231),
        primary_hover: rgb(244, 244, 245),
        primary_light: rgb(212, 212, 216),
        primary_border: rgb(255, 255, 255),
        text_on_primary: rgb(15, 15, 18),
        preview_dot: rgb(228, 228, 231),
    },
];

pub const OPACITY_PRESETS: [u8; 5] = [100, 95, 90, 85, 75];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    #[serde(default)]
    pub hotkey_index: usize, // 0: Alt+Space, 1: Ctrl+Space, 2: Ctrl+K, 3: F2
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub theme_index: usize,
    #[serde(default = "default_opacity")]
    pub opacity_percent: u8,
    #[serde(default = "default_aliases")]
    pub custom_aliases: HashMap<String, String>,
    #[serde(default = "default_snippets")]
    pub custom_snippets: HashMap<String, String>,
    #[serde(default = "default_update_url")]
    pub update_url: String,
    #[serde(default = "default_auto_update")]
    pub auto_update: bool,
}

fn default_auto_update() -> bool {
    true
}

fn default_update_url() -> String {
    "https://raw.githubusercontent.com/castinio/litecast/main/version.json".to_string()
}

fn default_opacity() -> u8 {
    95
}

fn default_snippets() -> HashMap<String, String> {
    let mut snips = HashMap::new();
    snips.insert(
        "iban".to_string(),
        "TR00 0000 0000 0000 0000 0000 00".to_string(),
    );
    snips.insert("mail".to_string(), "ornek@eposta.com".to_string());
    snips.insert("tel".to_string(), "+90 555 123 4567".to_string());
    snips
}

fn default_aliases() -> HashMap<String, String> {
    let mut custom_aliases = HashMap::new();
    custom_aliases.insert("yt".to_string(), "YouTube".to_string());
    custom_aliases.insert("sp".to_string(), "Spotify".to_string());
    custom_aliases.insert("st".to_string(), "Steam".to_string());
    custom_aliases.insert("dc".to_string(), "Discord".to_string());
    custom_aliases.insert("wa".to_string(), "WhatsApp".to_string());
    custom_aliases.insert("gh".to_string(), "GitHub".to_string());
    custom_aliases.insert("cp".to_string(), "Control Panel".to_string());
    custom_aliases.insert("calc".to_string(), "Calculator".to_string());
    custom_aliases.insert("np".to_string(), "Notepad".to_string());
    custom_aliases.insert("ps".to_string(), "PowerShell".to_string());
    custom_aliases.insert("cmd".to_string(), "Command Prompt".to_string());
    custom_aliases
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey_index: 0,
            autostart: false,
            theme_index: 0,
            opacity_percent: 95,
            custom_aliases: default_aliases(),
            custom_snippets: default_snippets(),
            update_url: default_update_url(),
            auto_update: true,
        }
    }
}

pub fn get_app_storage_dir() -> PathBuf {
    // Check if portable mode is enabled via portable.txt or a local data directory next to the exe
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        let portable_marker = exe_dir.join("portable.txt");
        let local_data = exe_dir.join("data");
        if portable_marker.exists() || local_data.exists() {
            let _ = create_dir_all(&local_data);
            return local_data;
        }
    }

    let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(local_app_data).join("LiteCast");
    let _ = create_dir_all(&dir);
    dir
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        get_app_storage_dir().join("config.json")
    }

    pub fn load() -> Self {
        if let Ok(file) = File::open(Self::config_path()) {
            let reader = BufReader::new(file);
            if let Ok(cfg) = serde_json::from_reader(reader) {
                return cfg;
            }
        }
        let default_cfg = Self::default();
        default_cfg.save();
        default_cfg
    }

    pub fn save(&self) {
        if let Ok(file) = File::create(Self::config_path()) {
            let writer = BufWriter::new(file);
            let _ = serde_json::to_writer_pretty(writer, self);
        }
    }

    pub fn theme(&self) -> ThemeColors {
        if self.theme_index < THEME_PRESETS.len() {
            THEME_PRESETS[self.theme_index]
        } else {
            THEME_PRESETS[0]
        }
    }

    pub fn hotkey_name(&self) -> &'static str {
        match self.hotkey_index {
            0 => "Alt + Space",
            1 => "Ctrl + Space",
            2 => "Ctrl + K",
            3 => "F2",
            _ => "Alt + Space",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.hotkey_index, 0);
        assert_eq!(cfg.hotkey_name(), "Alt + Space");
        assert_eq!(cfg.opacity_percent, 95);
        assert_eq!(cfg.theme_index, 0);
        assert!(cfg.custom_aliases.contains_key("yt"));
        assert!(cfg.custom_aliases.contains_key("sp"));
    }
}
