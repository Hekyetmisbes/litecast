// Extensible Script & JSON Plugin System for LiteCast.

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::config::get_app_storage_dir;
use crate::search::{ResultAction, SearchResult};

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginOutputItem {
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default = "default_badge")]
    pub badge: String,
    #[serde(default = "default_action")]
    pub action: String, // "copy", "open", "launch"
    pub value: String,
}

fn default_badge() -> String {
    "Eklenti".to_string()
}

fn default_action() -> String {
    "copy".to_string()
}

pub fn get_plugins_dir() -> PathBuf {
    let dir = get_app_storage_dir().join("plugins");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn search_plugins(input: &str) -> Vec<SearchResult> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let is_explicit_plugin_query = trimmed == "plugin"
        || trimmed == "plugins"
        || trimmed == "eklenti"
        || trimmed == "eklentiler"
        || trimmed.starts_with("p:")
        || trimmed.starts_with("plugin ");

    if !is_explicit_plugin_query {
        return Vec::new();
    }

    let plugins_dir = get_plugins_dir();

    if trimmed == "plugin"
        || trimmed == "plugins"
        || trimmed == "eklenti"
        || trimmed == "eklentiler"
    {
        return vec![SearchResult {
            id: "plugin_guide".to_string(),
            title: "🔌 LiteCast Eklenti & Betik Klasörü".to_string(),
            subtitle: format!(
                "{}  •  Enter ile eklenti klasörünü açın",
                plugins_dir.display()
            ),
            badge: "Eklentiler".to_string(),
            score: 8_500,
            action: ResultAction::LaunchApp {
                target: "explorer.exe".to_string(),
                args: Some(plugins_dir.to_string_lossy().to_string()),
                item_id: "plugin_guide".to_string(),
            },
            color_preview: None,
        }];
    }

    let query_payload = if let Some(rest) = trimmed.strip_prefix("p:") {
        rest.trim()
    } else if let Some(rest) = trimmed.strip_prefix("plugin ") {
        rest.trim()
    } else {
        trimmed
    };

    let mut results = Vec::new();

    // Look for matching script in plugins directory
    if let Ok(entries) = fs::read_dir(&plugins_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let file_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            let matches_plugin = query_payload.to_lowercase().starts_with(&file_stem);
            if matches_plugin {
                let script_args = query_payload[file_stem.len()..].trim();
                let output_items = execute_plugin_script(&path, &ext, script_args);

                for (idx, item) in output_items.into_iter().enumerate() {
                    let action = match item.action.as_str() {
                        "open" => ResultAction::OpenUrl {
                            url: item.value.clone(),
                        },
                        "launch" => ResultAction::LaunchApp {
                            target: item.value.clone(),
                            args: None,
                            item_id: format!("plugin_{}_{}", file_stem, idx),
                        },
                        _ => ResultAction::CopyToClipboard {
                            text: item.value.clone(),
                            notify_msg: format!("Kopyalandı: {}", item.value),
                        },
                    };

                    results.push(SearchResult {
                        id: format!("plugin_{}_{}", file_stem, idx),
                        title: item.title,
                        subtitle: item.subtitle,
                        badge: item.badge,
                        score: 9_100 - idx as i32,
                        action,
                        color_preview: None,
                    });
                }
            }
        }
    }

    results
}

fn execute_plugin_script(script_path: &Path, ext: &str, args: &str) -> Vec<PluginOutputItem> {
    let mut cmd = match ext {
        "bat" | "cmd" => {
            let mut c = Command::new("cmd.exe");
            c.args(["/C", &script_path.to_string_lossy(), args]);
            c
        }
        "ps1" => {
            let mut c = Command::new("powershell.exe");
            c.args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                &script_path.to_string_lossy(),
                args,
            ]);
            c
        }
        "py" => {
            let mut c = Command::new("python");
            c.args([script_path.to_string_lossy().as_ref(), args]);
            c
        }
        "exe" => {
            let mut c = Command::new(script_path);
            if !args.is_empty() {
                c.arg(args);
            }
            c
        }
        _ => return Vec::new(),
    };

    cmd.creation_flags(CREATE_NO_WINDOW);

    if let Ok(output) = cmd.output()
        && output.status.success()
    {
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        if let Ok(items) = serde_json::from_str::<Vec<PluginOutputItem>>(&stdout_str) {
            return items;
        }
        // Fallback plain text output
        let line = stdout_str.trim();
        if !line.is_empty() {
            return vec![PluginOutputItem {
                title: line.to_string(),
                subtitle: "Betik çıktısı  •  Enter ile kopyalayın".to_string(),
                badge: "Eklenti".to_string(),
                action: "copy".to_string(),
                value: line.to_string(),
            }];
        }
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_query_guide() {
        let res = search_plugins("plugin");
        assert_eq!(res.len(), 1);
        assert!(res[0].title.contains("Eklenti"));
    }
}
