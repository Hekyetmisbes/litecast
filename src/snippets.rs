// Quick Text Snippets & Dynamic Expansions for LiteCast.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetItem {
    pub trigger: String,
    pub title: String,
    pub preview: String,
    pub content: String,
    pub is_dynamic: bool,
}

pub fn get_builtin_snippets() -> Vec<SnippetItem> {
    vec![
        SnippetItem {
            trigger: "!date".to_string(),
            title: "📅 Bugünün Tarihi".to_string(),
            preview: "Gün/Ay/Yıl".to_string(),
            content: String::new(), // Evaluated dynamically
            is_dynamic: true,
        },
        SnippetItem {
            trigger: "!time".to_string(),
            title: "⏰ Şu Anki Saat".to_string(),
            preview: "Saat:Dakika:Saniye".to_string(),
            content: String::new(),
            is_dynamic: true,
        },
        SnippetItem {
            trigger: "!datetime".to_string(),
            title: "🗓️ Tarih ve Saat".to_string(),
            preview: "Tarih ve Saat Damgası".to_string(),
            content: String::new(),
            is_dynamic: true,
        },
        SnippetItem {
            trigger: "!shrug".to_string(),
            title: "¯\\_(ツ)_/¯".to_string(),
            preview: "Shrug Kaomoji".to_string(),
            content: "¯\\_(ツ)_/¯".to_string(),
            is_dynamic: false,
        },
        SnippetItem {
            trigger: "!tableflip".to_string(),
            title: "(╯°□°)╯︵ ┻━┻".to_string(),
            preview: "Table Flip Kaomoji".to_string(),
            content: "(╯°□°)╯︵ ┻━┻".to_string(),
            is_dynamic: false,
        },
        SnippetItem {
            trigger: "!lorem".to_string(),
            title: "📜 Lorem Ipsum Metni".to_string(),
            preview: "Lorem ipsum dolor sit amet, consectetur...".to_string(),
            content: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.".to_string(),
            is_dynamic: false,
        },
    ]
}

pub fn evaluate_dynamic_snippet(trigger: &str) -> String {
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();

    // Approximate UTC / Local conversion (UTC+3 for Turkey standard or standard offset)
    let tz_offset = 3 * 3600; // Turkey Time is UTC+3
    let local_secs = total_secs + tz_offset;

    let sec = local_secs % 60;
    let min = (local_secs / 60) % 60;
    let hour = (local_secs / 3600) % 24;

    // Days calculation (from 1970-01-01)
    let days = local_secs / 86400;
    let (year, month, day) = days_to_ymd(days);

    match trigger {
        "!date" => format!("{:02}.{:02}.{:04}", day, month, year),
        "!time" => format!("{:02}:{:02}:{:02}", hour, min, sec),
        "!datetime" => format!("{:02}.{:02}.{:04} {:02}:{:02}", day, month, year, hour, min),
        _ => String::new(),
    }
}

fn days_to_ymd(days: u64) -> (u32, u32, u32) {
    // Gregorian calendar calculation
    let d = days as i64 + 719468;
    let era = if d >= 0 { d } else { d - 146096 } / 146097;
    let doe = (d - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (year as u32, month, day)
}

pub fn search_snippets(query: &str, custom_snippets: &HashMap<String, String>) -> Vec<SnippetItem> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }

    let is_snippet_search = q.starts_with('!') || q == "snip" || q == "snippet" || q == "snippets";
    let mut results = Vec::new();

    // 1. Built-in Snippets
    for mut s in get_builtin_snippets() {
        if is_snippet_search
            || s.trigger.to_lowercase().starts_with(&q)
            || s.title.to_lowercase().contains(&q)
        {
            if s.is_dynamic {
                s.content = evaluate_dynamic_snippet(&s.trigger);
                s.preview = s.content.clone();
            }
            results.push(s);
        }
    }

    // 2. Custom Snippets from config
    for (k, v) in custom_snippets {
        let trigger = if k.starts_with('!') {
            k.clone()
        } else {
            format!("!{}", k)
        };

        if is_snippet_search
            || trigger.to_lowercase().starts_with(&q)
            || k.to_lowercase().contains(&q)
            || v.to_lowercase().contains(&q)
        {
            let preview = if v.len() > 50 {
                format!("{}...", &v[..47])
            } else {
                v.clone()
            };

            results.push(SnippetItem {
                trigger,
                title: format!("📝 Snippet: {}", k),
                preview,
                content: v.clone(),
                is_dynamic: false,
            });
        }
    }

    results
}
