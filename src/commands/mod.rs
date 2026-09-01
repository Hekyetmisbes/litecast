pub mod help;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAction {
    OpenHelp,
    OpenInstallHelp,
    OpenHotkeysHelp,
    OpenSettings,
    ToggleAutostart,
    EnableAutostart,
    DisableAutostart,
    Reindex,
    Quit,
}

pub struct InternalCommand {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub action: CommandAction,
}

pub fn parse_command(input: &str) -> Option<CommandAction> {
    let trimmed = input.trim().to_lowercase();
    match trimmed.as_str() {
        "settings" | "setting" | "ayarlar" | "ayar" | "config" | "control panel" => {
            Some(CommandAction::OpenSettings)
        }
        "help" => Some(CommandAction::OpenHelp),
        "help install" | "install" => Some(CommandAction::OpenInstallHelp),
        "hotkey" | "hotkeys" | "shortcuts" | "alias" | "aliases" => {
            Some(CommandAction::OpenHotkeysHelp)
        }
        "autostart on" | "autostart enable" | "startup on" | "startup enable" => {
            Some(CommandAction::EnableAutostart)
        }
        "autostart off" | "autostart disable" | "startup off" | "startup disable" => {
            Some(CommandAction::DisableAutostart)
        }
        "autostart" | "startup" | "autostart toggle" | "startup toggle" => {
            Some(CommandAction::ToggleAutostart)
        }
        "reindex" => Some(CommandAction::Reindex),
        "quit" | "exit" => Some(CommandAction::Quit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_known_commands() {
        assert_eq!(parse_command("help"), Some(CommandAction::OpenHelp));
        assert_eq!(parse_command("  HELP  "), Some(CommandAction::OpenHelp));
        assert_eq!(
            parse_command("help install"),
            Some(CommandAction::OpenInstallHelp)
        );
        assert_eq!(
            parse_command("install"),
            Some(CommandAction::OpenInstallHelp)
        );
        assert_eq!(
            parse_command("hotkey"),
            Some(CommandAction::OpenHotkeysHelp)
        );
        assert_eq!(
            parse_command("shortcuts"),
            Some(CommandAction::OpenHotkeysHelp)
        );
        assert_eq!(
            parse_command("aliases"),
            Some(CommandAction::OpenHotkeysHelp)
        );
        assert_eq!(
            parse_command("autostart"),
            Some(CommandAction::ToggleAutostart)
        );
        assert_eq!(
            parse_command("autostart on"),
            Some(CommandAction::EnableAutostart)
        );
        assert_eq!(
            parse_command("autostart off"),
            Some(CommandAction::DisableAutostart)
        );
        assert_eq!(parse_command("reindex"), Some(CommandAction::Reindex));
        assert_eq!(parse_command("quit"), Some(CommandAction::Quit));
        assert_eq!(parse_command("exit"), Some(CommandAction::Quit));
    }

    #[test]
    fn test_unknown_commands_return_none() {
        assert_eq!(parse_command("notepad"), None);
        assert_eq!(parse_command("help unknown"), None);
    }
}
