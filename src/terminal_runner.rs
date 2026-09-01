// Inline Terminal & Shell Command Runner for LiteCast.

use crate::search::{ResultAction, SearchResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellType {
    Cmd,
    PowerShell,
}

pub fn parse_terminal_command(input: &str) -> Vec<SearchResult> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let (cmd_str, shell_type) = if let Some(rest) = trimmed.strip_prefix('>') {
        (rest.trim(), ShellType::Cmd)
    } else if let Some(rest) = trimmed.strip_prefix('$') {
        (rest.trim(), ShellType::PowerShell)
    } else if let Some(rest) = trimmed.strip_prefix("cmd:") {
        (rest.trim(), ShellType::Cmd)
    } else if let Some(rest) = trimmed.strip_prefix("ps:") {
        (rest.trim(), ShellType::PowerShell)
    } else {
        return Vec::new();
    };

    if cmd_str.is_empty() {
        return vec![SearchResult {
            id: "terminal_hint".to_string(),
            title: "💻 Terminal Komutu Çalıştır".to_string(),
            subtitle:
                "Çalıştırmak istediğiniz komutu yazın (örn: > ping google.com veya $ Get-Process)"
                    .to_string(),
            badge: "Terminal".to_string(),
            score: 11_000,
            action: ResultAction::LaunchApp {
                target: "wt.exe".to_string(),
                args: None,
                item_id: "terminal_hint".to_string(),
            },
            color_preview: None,
        }];
    }

    let mut results = Vec::new();

    let (target_bin, default_args, elev_args) = match shell_type {
        ShellType::Cmd => (
            "cmd.exe",
            format!("/k \"{}\"", cmd_str),
            format!("/k \"{}\"", cmd_str),
        ),
        ShellType::PowerShell => (
            "powershell.exe",
            format!("-NoExit -Command \"{}\"", cmd_str),
            format!("-NoExit -Command \"{}\"", cmd_str),
        ),
    };

    // Primary: Normal Terminal Execution
    results.push(SearchResult {
        id: format!("term_run_{}", cmd_str),
        title: format!("💻 Komutu Çalıştır: {}", cmd_str),
        subtitle: format!("{} {}  •  Enter ile çalıştırın", target_bin, default_args),
        badge: match shell_type {
            ShellType::Cmd => "CMD / Konsol",
            ShellType::PowerShell => "PowerShell",
        }
        .to_string(),
        score: 11_000,
        action: ResultAction::LaunchApp {
            target: target_bin.to_string(),
            args: Some(default_args),
            item_id: format!("term_run_{}", cmd_str),
        },
        color_preview: None,
    });

    // Secondary: Elevated Execution (As Admin)
    results.push(SearchResult {
        id: format!("term_admin_{}", cmd_str),
        title: format!("🛡️ Yönetici Olarak Çalıştır: {}", cmd_str),
        subtitle: format!(
            "{} {}  •  Yönetici ayrıcalıkları ile",
            target_bin, elev_args
        ),
        badge: "Yönetici".to_string(),
        score: 10_900,
        action: ResultAction::LaunchElevated {
            target: target_bin.to_string(),
            args: Some(elev_args),
            working_dir: None,
        },
        color_preview: None,
    });

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_terminal_command() {
        let results = parse_terminal_command("> ping 1.1.1.1");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].badge, "CMD / Konsol");
        assert!(results[0].title.contains("ping 1.1.1.1"));

        let ps_results = parse_terminal_command("$ Get-Service");
        assert_eq!(ps_results.len(), 2);
        assert_eq!(ps_results[0].badge, "PowerShell");
    }
}
