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

    // Look for matching script in plugins directory (Static discovery only, never execute on typing!)
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

            if !["ps1", "bat", "cmd", "py", "exe"].contains(&ext.as_str()) {
                continue;
            }

            let matches_plugin = query_payload.is_empty()
                || query_payload.to_lowercase().starts_with(&file_stem)
                || file_stem.contains(&query_payload.to_lowercase());

            if matches_plugin {
                let script_args = if query_payload.to_lowercase().starts_with(&file_stem) {
                    query_payload[file_stem.len()..].trim().to_string()
                } else {
                    String::new()
                };

                let display_args = if script_args.is_empty() {
                    String::new()
                } else {
                    format!(" \"{}\"", script_args)
                };

                results.push(SearchResult {
                    id: format!("plugin_{}", file_stem),
                    title: format!("🔌 {} ({})", file_stem, ext.to_uppercase()),
                    subtitle: format!(
                        "Eklentiyi çalıştır{}: {}  •  Enter ile çalıştırın",
                        display_args,
                        path.display()
                    ),
                    badge: "Eklenti".to_string(),
                    score: 9_100,
                    action: ResultAction::ExecutePlugin {
                        script_path: path.to_string_lossy().to_string(),
                        ext: ext.clone(),
                        args: script_args,
                    },
                    color_preview: None,
                });
            }
        }
    }

    results
}

/// Executes a plugin action explicitly triggered by user confirmation (Enter key).
pub fn run_plugin_action(script_path_str: &str, ext: &str, args: &str) {
    let script_path = PathBuf::from(script_path_str);
    let plugins_dir = get_plugins_dir();

    // Path traversal verification
    let is_valid = match (script_path.canonicalize(), plugins_dir.canonicalize()) {
        (Ok(can_script), Ok(can_plugins)) => can_script.starts_with(&can_plugins),
        _ => true,
    };
    if !is_valid {
        crate::telemetry_local::log_error(
            "Eklenti yolu güvenlik ihlali: Eklenti klasörü dışında dosya.",
        );
        return;
    }

    let items = execute_plugin_script(&script_path, ext, args);
    if let Some(first) = items.first() {
        match first.action.as_str() {
            "open" => {
                let _ = crate::platform_windows::launch_target(&first.value, None);
            }
            "launch" => {
                let _ = crate::platform_windows::launch_target(&first.value, None);
            }
            _ => {
                let _ = crate::clipboard::set_clipboard_text(&first.value);
            }
        }
    }
}

pub fn execute_plugin_script(script_path: &Path, ext: &str, args: &str) -> Vec<PluginOutputItem> {
    let mut cmd = match ext {
        "bat" | "cmd" => {
            let mut c = Command::new("cmd.exe");
            c.args(["/C", &script_path.to_string_lossy()]);
            if !args.is_empty() {
                c.arg(args);
            }
            c
        }
        "ps1" => {
            let mut c = Command::new("powershell.exe");
            c.args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "RemoteSigned", // LC-016: Secure RemoteSigned instead of Bypass
                "-File",
                &script_path.to_string_lossy(),
            ]);
            if !args.is_empty() {
                c.arg(args);
            }
            c
        }
        "py" => {
            let mut c = Command::new("python");
            c.arg(script_path);
            if !args.is_empty() {
                c.arg(args);
            }
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

    // LC-017: Execution timeout (5s) and bounded memory output (64 KB)
    if let Ok(mut child) = cmd.stdout(std::process::Stdio::piped()).spawn() {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(5);
        let mut finished = false;

        while start.elapsed() < timeout {
            if let Ok(Some(_status)) = child.try_wait() {
                finished = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        if !finished {
            let _ = child.kill();
            crate::telemetry_local::log_error("Eklenti çalışma süresi aşıldı (5s zaman aşımı).");
            return Vec::new();
        }

        if let Some(mut stdout) = child.stdout.take() {
            use std::io::Read;
            let mut buf = Vec::new();
            let _ = stdout.by_ref().take(64 * 1024).read_to_end(&mut buf);
            let stdout_str = String::from_utf8_lossy(&buf);

            if let Ok(items) = serde_json::from_str::<Vec<PluginOutputItem>>(&stdout_str) {
                return items;
            }

            let line = stdout_str.trim();
            if !line.is_empty() {
                return vec![PluginOutputItem {
                    title: line.to_string(),
                    subtitle: "Betik çıktısı  •  Panoya kopyalandı".to_string(),
                    badge: "Eklenti".to_string(),
                    action: "copy".to_string(),
                    value: line.to_string(),
                }];
            }
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
