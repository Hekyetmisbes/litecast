// Unified Intelligent Search Pipeline for LiteCast.

use windows::Win32::Foundation::COLORREF;

use crate::calculator::evaluate_math_expression;
use crate::clipboard::CLIPBOARD_HISTORY;
use crate::config::AppConfig;
use crate::converters::search_converters;
use crate::dev_tools::parse_dev_tools;
use crate::file_browser::{is_path_query, search_file_browser};
use crate::history::UsageHistory;
use crate::indexer::AppItem;
use crate::plugins::search_plugins;
use crate::snippets::search_snippets;
use crate::system_actions::{SystemActionKind, search_system_actions};
use crate::terminal_runner::parse_terminal_command;
use crate::web_search::{make_web_fallback, parse_direct_url, parse_web_bang};
use crate::window_switcher::search_open_windows;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultAction {
    LaunchApp {
        target: String,
        args: Option<String>,
        item_id: String,
    },
    LaunchElevated {
        target: String,
        args: Option<String>,
        working_dir: Option<String>,
    },
    CopyToClipboard {
        text: String,
        notify_msg: String,
    },
    OpenUrl {
        url: String,
    },
    ExecuteSystem {
        kind: SystemActionKind,
    },
    FocusWindow {
        hwnd_val: isize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub badge: String,
    pub score: i32,
    pub action: ResultAction,
    pub color_preview: Option<COLORREF>,
}

pub fn normalize_query(query: &str) -> String {
    query.trim().to_lowercase()
}

pub fn check_alias_match(query: &str, app_name: &str) -> Option<i32> {
    let q = query.trim().to_lowercase();
    let name = app_name.to_lowercase();

    match q.as_str() {
        "pcm" | "pcmanager" | "pc manager" if name.contains("pc manager") => Some(2000),
        "store" | "msstore" | "ms store" | "microsoft store"
            if name.contains("microsoft store") || name.contains("store") =>
        {
            Some(2000)
        }
        "yt" if name.contains("youtube") => Some(2000),
        "sp" if name.contains("spotify") => Some(2000),
        "st" if name.contains("steam") => Some(2000),
        "dc" if name.contains("discord") => Some(2000),
        "wa" if name.contains("whatsapp") => Some(2000),
        "gh" if name.contains("github") => Some(2000),
        "gpt" | "chatgpt" | "ai" if name.contains("chatgpt") || name.contains("openai") => {
            Some(2000)
        }
        "cp" | "cm" if name.contains("control panel") || name.contains("denetim masası") => {
            Some(2000)
        }
        "np" if name.contains("notepad") || name.contains("not defteri") => Some(2000),
        "ps" if name.contains("powershell") => Some(2000),
        "calc" if name.contains("calculator") || name.contains("hesap makinesi") => Some(2000),
        "cmd" if name.contains("command prompt") || name.contains("komut istemi") => Some(2000),
        // Gaming aliases
        "cs" | "cs2" | "csgo" if name.contains("counter-strike") => Some(2000),
        "val" | "valo" if name.contains("valorant") => Some(2000),
        "lol" if name.contains("league of legends") => Some(2000),
        "ets" | "ets2" if name.contains("euro truck") => Some(2000),
        "fm" | "fm24" | "fm2024" if name.contains("football manager") => Some(2000),
        "mc" if name.contains("minecraft") => Some(2000),
        "wd" | "wd2" if name.contains("watch dogs") || name.contains("watch_dogs") => Some(2000),
        "lor" if name.contains("legends of runeterra") => Some(2000),
        _ => None,
    }
}

pub fn fuzzy_score(pattern: &str, text: &str) -> Option<i32> {
    let p_norm = normalize_query(pattern);
    let t_norm = normalize_query(text);

    if p_norm.is_empty() {
        return Some(0);
    }
    if t_norm.is_empty() {
        return None;
    }

    if let Some(alias_boost) = check_alias_match(&p_norm, &t_norm) {
        return Some(alias_boost);
    }

    if t_norm == p_norm {
        return Some(1000);
    }

    if t_norm.starts_with(&p_norm) {
        return Some(500 + (100 - (t_norm.len() as i32 - p_norm.len() as i32).clamp(0, 100)));
    }

    // Word prefix match
    for word in t_norm.split_whitespace() {
        if word.starts_with(&p_norm) {
            return Some(400 + (50 - (t_norm.len() as i32).clamp(0, 50)));
        }
    }

    // Subsequence matching
    let mut p_chars = p_norm.chars().peekable();
    let mut matches = 0;
    let mut prev_idx = 0;
    let mut consecutive_bonus = 0;

    for (i, c) in t_norm.chars().enumerate() {
        if let Some(&pc) = p_chars.peek()
            && c == pc
        {
            p_chars.next();
            matches += 1;
            if matches > 1 && i == prev_idx + 1 {
                consecutive_bonus += 15;
            }
            prev_idx = i;
        }
    }

    if p_chars.peek().is_none() {
        Some(100 + matches * 10 + consecutive_bonus - (t_norm.len() as i32))
    } else {
        None
    }
}

pub fn search_apps(items: &[AppItem], query: &str, limit: usize) -> Vec<SearchResult> {
    let cfg = AppConfig::load();
    search_apps_with_history(items, query, &UsageHistory::default(), &cfg, limit)
}

pub fn search_apps_with_history(
    items: &[AppItem],
    query: &str,
    history: &UsageHistory,
    config: &AppConfig,
    limit: usize,
) -> Vec<SearchResult> {
    let trimmed = query.trim();
    let norm = normalize_query(query);

    let mut unified_results: Vec<SearchResult> = Vec::new();

    // -------------------------------------------------------------
    // 1. Terminal / Shell Command Runner (>, $, cmd:, ps:)
    // -------------------------------------------------------------
    let term_results = parse_terminal_command(trimmed);
    if !term_results.is_empty() {
        unified_results.extend(term_results);
    }

    // -------------------------------------------------------------
    // 2. Converters (Units, Timezones, Unix Epoch, Currency/Crypto)
    // -------------------------------------------------------------
    let conv_results = search_converters(trimmed);
    if !conv_results.is_empty() {
        unified_results.extend(conv_results);
    }

    // -------------------------------------------------------------
    // 3. Inline File & Directory Browser (C:\, D:\, %appdata%, ~/)
    // -------------------------------------------------------------
    if is_path_query(trimmed) {
        let file_results = search_file_browser(trimmed, limit);
        if !file_results.is_empty() {
            unified_results.extend(file_results);
        }
    }

    // -------------------------------------------------------------
    // 4. Math / Calculator Evaluation
    // -------------------------------------------------------------
    if let Some(calc) = evaluate_math_expression(trimmed) {
        unified_results.push(SearchResult {
            id: "calc_result".to_string(),
            title: format!("🧮 Hesaplama Sonucu: {}", calc.formatted_result),
            subtitle: format!("İfade: {}  •  Enter ile panoya kopyalayın", calc.expression),
            badge: "Hesap Makinesi".to_string(),
            score: 10_000,
            action: ResultAction::CopyToClipboard {
                text: calc.formatted_result.clone(),
                notify_msg: format!("Sonuç panoya kopyalandı: {}", calc.formatted_result),
            },
            color_preview: None,
        });
    }

    // -------------------------------------------------------------
    // 5. Direct Web URL Navigation (github.com, https://...)
    // -------------------------------------------------------------
    if let Some(url_match) = parse_direct_url(trimmed) {
        unified_results.push(SearchResult {
            id: format!("direct_url_{}", url_match.target_url),
            title: url_match.title,
            subtitle: format!("URL: {}", url_match.target_url),
            badge: url_match.badge.to_string(),
            score: 9_800,
            action: ResultAction::OpenUrl {
                url: url_match.target_url,
            },
            color_preview: None,
        });
    }

    // -------------------------------------------------------------
    // 6. Web Search Bangs (g, yt, gh, wiki, tr, chat, ddg...)
    // -------------------------------------------------------------
    if let Some(bang) = parse_web_bang(trimmed) {
        unified_results.push(SearchResult {
            id: format!("web_bang_{}", bang.engine_name),
            title: bang.title,
            subtitle: format!("URL: {}", bang.target_url),
            badge: bang.badge.to_string(),
            score: 9_500,
            action: ResultAction::OpenUrl {
                url: bang.target_url,
            },
            color_preview: None,
        });
    }

    // -------------------------------------------------------------
    // 7. Extensible Plugins & Scripts
    // -------------------------------------------------------------
    let plugin_results = search_plugins(trimmed);
    if !plugin_results.is_empty() {
        unified_results.extend(plugin_results);
    }

    // -------------------------------------------------------------
    // 8. Developer & Utility Tools (JSON, JWT, Colors, Base64, UUID, Hashes, Len, Lorem, Cron)
    // -------------------------------------------------------------
    let dev_matches = parse_dev_tools(trimmed);
    for dev in dev_matches {
        unified_results.push(SearchResult {
            id: dev.id,
            title: dev.title,
            subtitle: dev.preview,
            badge: dev.badge.to_string(),
            score: 9_000,
            action: ResultAction::CopyToClipboard {
                text: dev.copy_value.clone(),
                notify_msg: format!("Panoya kopyalandı: {}", dev.copy_value),
            },
            color_preview: dev.color_preview,
        });
    }

    // -------------------------------------------------------------
    // 9. Quick Snippets & Text Templates
    // -------------------------------------------------------------
    let snippets = search_snippets(trimmed, &config.custom_snippets);
    for snip in snippets {
        unified_results.push(SearchResult {
            id: format!("snip_{}", snip.trigger),
            title: snip.title,
            subtitle: format!("İçerik: \"{}\"  •  Enter ile kopyalayın", snip.preview),
            badge: "Snippet".to_string(),
            score: 8_500,
            action: ResultAction::CopyToClipboard {
                text: snip.content.clone(),
                notify_msg: format!("Snippet kopyalandı: {}", snip.trigger),
            },
            color_preview: None,
        });
    }

    // -------------------------------------------------------------
    // 10. Windows Quick System & Power Actions
    // -------------------------------------------------------------
    let sys_actions = search_system_actions(trimmed);
    for sys in sys_actions {
        unified_results.push(SearchResult {
            id: sys.id.to_string(),
            title: sys.name.to_string(),
            subtitle: sys.description.to_string(),
            badge: sys.badge.to_string(),
            score: 8_000,
            action: ResultAction::ExecuteSystem { kind: sys.kind },
            color_preview: None,
        });
    }

    // -------------------------------------------------------------
    // 6. Active Window Switcher (win:, tab:, win ...)
    // -------------------------------------------------------------
    let win_matches = search_open_windows(trimmed);
    for w in win_matches {
        unified_results.push(SearchResult {
            id: format!("win_{}", w.hwnd_val),
            title: w.title,
            subtitle: w.preview,
            badge: w.badge.to_string(),
            score: 7_500,
            action: ResultAction::FocusWindow {
                hwnd_val: w.hwnd_val,
            },
            color_preview: None,
        });
    }

    // -------------------------------------------------------------
    // 7. Clipboard History (cb, clip, pano)
    // -------------------------------------------------------------
    let is_clip_query = norm == "cb"
        || norm == "clip"
        || norm == "pano"
        || norm.starts_with("cb ")
        || norm.starts_with("clip ");
    if is_clip_query {
        let clean_clip_q = trimmed
            .strip_prefix("cb ")
            .or_else(|| trimmed.strip_prefix("clip "))
            .unwrap_or_default();

        let hist_items = {
            let guard = CLIPBOARD_HISTORY.lock().unwrap();
            guard
                .as_ref()
                .map(|h| h.search(clean_clip_q))
                .unwrap_or_default()
        };

        for (idx, item) in hist_items.iter().take(20).enumerate() {
            unified_results.push(SearchResult {
                id: item.id.clone(),
                title: format!("📋 {}", item.preview),
                subtitle: format!(
                    "{} karakter  •  Enter ile panoya geri yükleyin",
                    item.char_count
                ),
                badge: "Pano".to_string(),
                score: 7_000 - idx as i32,
                action: ResultAction::CopyToClipboard {
                    text: item.text.clone(),
                    notify_msg: "Pano öğesi panoya aktarıldı".to_string(),
                },
                color_preview: None,
            });
        }
    }

    // -------------------------------------------------------------
    // 8. Local Application Index (Fuzzy & Alias Search)
    // -------------------------------------------------------------
    if norm.is_empty() {
        let mut app_results: Vec<SearchResult> = items
            .iter()
            .map(|item| {
                let launches = history.get_launch_count(&item.id);
                let type_tag = if item.source_path.starts_with("system:") {
                    "Sistem Aracı"
                } else if item.source_path.starts_with("web:") {
                    "Web / PWA"
                } else if item.source_path.starts_with("protocol:") {
                    "Protokol"
                } else {
                    "Uygulama"
                };

                SearchResult {
                    id: item.id.clone(),
                    title: item.name.clone(),
                    subtitle: item.target_path.clone(),
                    badge: type_tag.to_string(),
                    score: launches as i32,
                    action: ResultAction::LaunchApp {
                        target: item.target_path.clone(),
                        args: item.arguments.clone(),
                        item_id: item.id.clone(),
                    },
                    color_preview: None,
                }
            })
            .collect();

        app_results.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });

        app_results.truncate(limit);
        return app_results;
    }

    for item in items {
        if let Some(base_score) = fuzzy_score(&norm, &item.name) {
            let launches = history.get_launch_count(&item.id);
            let history_boost = (launches as i32 * 5).min(50);

            let type_tag = if item.source_path.starts_with("system:") {
                "Sistem Aracı"
            } else if item.source_path.starts_with("web:") {
                "Web / PWA"
            } else if item.source_path.starts_with("protocol:") {
                "Protokol"
            } else {
                "Uygulama"
            };

            unified_results.push(SearchResult {
                id: item.id.clone(),
                title: item.name.clone(),
                subtitle: item.target_path.clone(),
                badge: type_tag.to_string(),
                score: base_score + history_boost,
                action: ResultAction::LaunchApp {
                    target: item.target_path.clone(),
                    args: item.arguments.clone(),
                    item_id: item.id.clone(),
                },
                color_preview: None,
            });
        }
    }

    // -------------------------------------------------------------
    // 9. Web Search Fallback (if query has enough chars)
    // -------------------------------------------------------------
    if !is_clip_query
        && trimmed.len() >= 2
        && !trimmed.starts_with('!')
        && let Some(fb) = make_web_fallback(trimmed)
    {
        unified_results.push(SearchResult {
            id: "web_fallback".to_string(),
            title: fb.title,
            subtitle: format!("Tarayıcıda Ara: {}", fb.target_url),
            badge: fb.badge.to_string(),
            score: 50, // Low priority fallback
            action: ResultAction::OpenUrl { url: fb.target_url },
            color_preview: None,
        });
    }

    // Sort all results deterministically by score descending
    unified_results.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });

    unified_results.truncate(limit);
    unified_results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalization() {
        assert_eq!(normalize_query("  Visual Studio  "), "visual studio");
        assert_eq!(normalize_query("Notepad"), "notepad");
    }

    #[test]
    fn test_quick_aliases() {
        assert_eq!(check_alias_match("yt", "YouTube"), Some(2000));
        assert_eq!(check_alias_match("sp", "Spotify"), Some(2000));
        assert_eq!(check_alias_match("st", "Steam"), Some(2000));
    }

    #[test]
    fn test_unified_calc_search() {
        let items = Vec::new();
        let cfg = AppConfig::default();
        let results =
            search_apps_with_history(&items, "25 * 4", &UsageHistory::default(), &cfg, 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].badge, "Hesap Makinesi");
        assert!(results[0].title.contains("100"));
    }

    #[test]
    fn test_unified_web_bang_search() {
        let items = Vec::new();
        let cfg = AppConfig::default();
        let results =
            search_apps_with_history(&items, "g rust lang", &UsageHistory::default(), &cfg, 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].badge, "Google Arama");
    }

    #[test]
    fn test_unified_color_search() {
        let items = Vec::new();
        let cfg = AppConfig::default();
        let results =
            search_apps_with_history(&items, "#3b82f6", &UsageHistory::default(), &cfg, 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].badge, "Renk Paleti");
    }
}
