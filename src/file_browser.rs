// Inline File & Directory Browser for LiteCast.

use std::fs;
use std::path::{Path, PathBuf};

use crate::search::{ResultAction, SearchResult};

pub fn is_path_query(input: &str) -> bool {
    let s = input.trim();
    if s.is_empty() {
        return false;
    }

    // Drive letter: C:\, D:\, C:/, D: etc.
    if s.len() >= 2
        && s.chars().next().unwrap_or(' ').is_ascii_alphabetic()
        && s.chars().nth(1) == Some(':')
    {
        return true;
    }

    // Environment variables or relative shortcuts
    s.starts_with('%')
        || s.starts_with('~')
        || s.starts_with('\\')
        || s.starts_with('/')
        || s.starts_with("./")
        || s.starts_with(".\\")
        || s.starts_with("../")
        || s.starts_with("..\\")
}

pub fn expand_env_path(input: &str) -> PathBuf {
    let s = input.trim();

    if let Some(rest) = s.strip_prefix('~') {
        let user_prof = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users".to_string());
        let clean_rest = rest.trim_start_matches(['/', '\\']);
        return if clean_rest.is_empty() {
            PathBuf::from(user_prof)
        } else {
            PathBuf::from(user_prof).join(clean_rest)
        };
    }

    if let Some(stripped) = s.strip_prefix('%')
        && let Some(second_pct) = stripped.find('%')
    {
        let var_name = &stripped[..second_pct];
        let rest = &stripped[second_pct + 1..].trim_start_matches(['/', '\\']);
        if let Ok(val) = std::env::var(var_name) {
            return if rest.is_empty() {
                PathBuf::from(val)
            } else {
                PathBuf::from(val).join(rest)
            };
        }
    }

    PathBuf::from(s)
}

fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn get_file_type_info(path: &Path, is_dir: bool) -> (&'static str, &'static str) {
    if is_dir {
        return ("📁", "Klasör");
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "exe" | "bat" | "cmd" | "ps1" | "msi" => ("⚡", "Çalıştırılabilir"),
        "txt" | "md" | "doc" | "docx" | "pdf" | "rtf" => ("📄", "Belge"),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "ico" => ("🖼️", "Resim"),
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => ("🎵", "Müzik"),
        "mp4" | "mkv" | "avi" | "mov" | "webm" => ("🎬", "Video"),
        "zip" | "rar" | "7z" | "tar" | "gz" => ("📦", "Arşiv"),
        "rs" | "py" | "js" | "ts" | "cpp" | "c" | "cs" | "html" | "css" | "json" | "toml" => {
            ("💻", "Kaynak Kod")
        }
        "lnk" | "url" => ("🔗", "Kısayol"),
        _ => ("📄", "Dosya"),
    }
}

pub fn search_file_browser(input: &str, limit: usize) -> Vec<SearchResult> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let target_path = expand_env_path(trimmed);
    let mut results = Vec::new();

    // Case 1: Exact directory that exists
    if target_path.is_dir() {
        if let Ok(entries) = fs::read_dir(&target_path) {
            let mut list = Vec::new();
            for entry in entries.flatten() {
                let p = entry.path();
                let is_dir = p.is_dir();
                let file_name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                if file_name.starts_with('$') || (file_name.starts_with('.') && file_name.len() > 1)
                {
                    // Skip internal system hidden entries like $Recycle.Bin
                    continue;
                }

                let size = if is_dir {
                    0
                } else {
                    entry.metadata().map(|m| m.len()).unwrap_or(0)
                };

                list.push((p, file_name, is_dir, size));
            }

            // Sort directories first, then alphabetical
            list.sort_by(|a, b| {
                b.2.cmp(&a.2)
                    .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
            });

            for (p, file_name, is_dir, size) in list.into_iter().take(limit) {
                let (icon, badge) = get_file_type_info(&p, is_dir);
                let p_str = p.to_string_lossy().to_string();
                let subtitle = if is_dir {
                    format!("{}  •  Enter ile açın", p_str)
                } else {
                    format!("{}  •  {}", p_str, format_file_size(size))
                };

                results.push(SearchResult {
                    id: format!("file_{}", p_str),
                    title: format!("{} {}", icon, file_name),
                    subtitle,
                    badge: badge.to_string(),
                    score: 9_200,
                    action: ResultAction::LaunchApp {
                        target: p_str.clone(),
                        args: None,
                        item_id: format!("file_{}", p_str),
                    },
                    color_preview: None,
                });
            }
        }
        return results;
    }

    // Case 2: Incomplete path, e.g. C:\Users\Doc
    if let Some(parent) = target_path.parent()
        && parent.is_dir()
    {
        let prefix = target_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        if let Ok(entries) = fs::read_dir(parent) {
            let mut list = Vec::new();
            for entry in entries.flatten() {
                let p = entry.path();
                let file_name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                if file_name.to_lowercase().starts_with(&prefix) {
                    let is_dir = p.is_dir();
                    let size = if is_dir {
                        0
                    } else {
                        entry.metadata().map(|m| m.len()).unwrap_or(0)
                    };
                    list.push((p, file_name, is_dir, size));
                }
            }

            list.sort_by(|a, b| {
                b.2.cmp(&a.2)
                    .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
            });

            for (p, file_name, is_dir, size) in list.into_iter().take(limit) {
                let (icon, badge) = get_file_type_info(&p, is_dir);
                let p_str = p.to_string_lossy().to_string();
                let subtitle = if is_dir {
                    format!("{}  •  Enter ile açın", p_str)
                } else {
                    format!("{}  •  {}", p_str, format_file_size(size))
                };

                results.push(SearchResult {
                    id: format!("file_{}", p_str),
                    title: format!("{} {}", icon, file_name),
                    subtitle,
                    badge: badge.to_string(),
                    score: 9_300,
                    action: ResultAction::LaunchApp {
                        target: p_str.clone(),
                        args: None,
                        item_id: format!("file_{}", p_str),
                    },
                    color_preview: None,
                });
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_path_query() {
        assert!(is_path_query("C:\\"));
        assert!(is_path_query("C:/Users"));
        assert!(is_path_query("%appdata%"));
        assert!(is_path_query("~/Desktop"));
        assert!(!is_path_query("chrome"));
        assert!(!is_path_query("spotify"));
    }

    #[test]
    fn test_expand_env_path() {
        let expanded = expand_env_path("~/test");
        assert!(!expanded.to_string_lossy().starts_with('~'));
    }

    #[test]
    fn test_file_size_format() {
        assert_eq!(format_file_size(500), "500 B");
        assert_eq!(format_file_size(2048), "2.0 KB");
        assert_eq!(format_file_size(1048576 * 5), "5.0 MB");
    }
}
