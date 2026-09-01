// Windows Quick System & Power Actions for LiteCast.

use std::process::Command;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Shell::{
    SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHEmptyRecycleBinW,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APPCOMMAND};
use windows::core::PCWSTR;

use crate::telemetry_local::log_info;

const APPCOMMAND_VOLUME_MUTE: isize = 8;
const APPCOMMAND_VOLUME_DOWN: isize = 9;
const APPCOMMAND_VOLUME_UP: isize = 10;
const HWND_BROADCAST: HWND = HWND(0xffff as *mut _);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemActionKind {
    Lock,
    Sleep,
    Restart,
    Shutdown,
    EmptyRecycleBin,
    ToggleMute,
    VolumeUp,
    VolumeDown,
    OpenTaskMgr,
    ShowLocalIp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemActionItem {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub kind: SystemActionKind,
    pub badge: &'static str,
}

pub const SYSTEM_ACTIONS: &[SystemActionItem] = &[
    SystemActionItem {
        id: "sys_lock",
        name: "🔒 Bilgisayarı Kilitle",
        description: "Windows oturumunu anında kilitler (LockWorkStation)",
        kind: SystemActionKind::Lock,
        badge: "Sistem",
    },
    SystemActionItem {
        id: "sys_sleep",
        name: "🌙 Uyku Modu",
        description: "Bilgisayarı uyku moduna alır",
        kind: SystemActionKind::Sleep,
        badge: "Güç",
    },
    SystemActionItem {
        id: "sys_restart",
        name: "🔄 Bilgisayarı Yeniden Başlat",
        description: "Windows'u hemen yeniden başlatır",
        kind: SystemActionKind::Restart,
        badge: "Güç",
    },
    SystemActionItem {
        id: "sys_shutdown",
        name: "🛑 Bilgisayarı Kapat",
        description: "Bilgisayarı güvenli şekilde kapatır",
        kind: SystemActionKind::Shutdown,
        badge: "Güç",
    },
    SystemActionItem {
        id: "sys_empty_bin",
        name: "🗑️ Geri Dönüşüm Kutusunu Boşalt",
        description: "Çöp kutusundaki tüm geçici dosyaları temizler",
        kind: SystemActionKind::EmptyRecycleBin,
        badge: "Temizlik",
    },
    SystemActionItem {
        id: "sys_mute",
        name: "🔇 Sesi Kapat / Aç (Mute)",
        description: "Windows ana sesini sessize alır veya açar",
        kind: SystemActionKind::ToggleMute,
        badge: "Ses",
    },
    SystemActionItem {
        id: "sys_volup",
        name: "🔊 Sesi Artır (+)",
        description: "Sistem sesini artırır",
        kind: SystemActionKind::VolumeUp,
        badge: "Ses",
    },
    SystemActionItem {
        id: "sys_voldown",
        name: "🔉 Sesi Azalt (-)",
        description: "Sistem sesini azaltır",
        kind: SystemActionKind::VolumeDown,
        badge: "Ses",
    },
    SystemActionItem {
        id: "sys_taskmgr",
        name: "⚡ Görev Yöneticisi (Task Manager)",
        description: "Windows Görev Yöneticisi'ni açar",
        kind: SystemActionKind::OpenTaskMgr,
        badge: "Sistem",
    },
    SystemActionItem {
        id: "sys_ip",
        name: "🌐 Yerel IP Adresini Göster / Kopyala",
        description: "Ağdaki yerel IPv4 adresini panoya kopyalar",
        kind: SystemActionKind::ShowLocalIp,
        badge: "Ağ",
    },
];

pub fn search_system_actions(query: &str) -> Vec<SystemActionItem> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    for action in SYSTEM_ACTIONS {
        let is_match = match (action.id, q.as_str()) {
            ("sys_lock", "lock" | "kilit" | "kilitle" | "oturum") => true,
            ("sys_sleep", "sleep" | "uyku" | "askiya al" | "uyut") => true,
            ("sys_restart", "restart" | "reboot" | "yeniden" | "yeniden baslat") => true,
            ("sys_shutdown", "shutdown" | "kapat" | "power off" | "bilgisayari kapat") => true,
            (
                "sys_empty_bin",
                "empty" | "empty bin" | "trash" | "cop" | "çöp" | "cop kutusu" | "geri donusum"
                | "recycle",
            ) => true,
            ("sys_mute", "mute" | "ses" | "sessiz" | "ses kapat" | "unmute") => true,
            ("sys_volup", "volup" | "ses artir" | "ses yükselt" | "volume up") => true,
            ("sys_voldown", "voldown" | "ses azalt" | "ses kis" | "volume down") => true,
            ("sys_taskmgr", "task" | "taskmgr" | "gorev" | "görev" | "performans" | "islem") => {
                true
            }
            ("sys_ip", "ip" | "ipconfig" | "myip" | "yerel ip" | "ag" | "network") => true,
            _ => {
                let name_clean = action.name.to_lowercase();
                let desc_clean = action.description.to_lowercase();
                name_clean.contains(&q) || desc_clean.contains(&q)
            }
        };

        if is_match {
            matches.push(action.clone());
        }
    }

    matches
}

pub fn execute_system_action(kind: &SystemActionKind) -> Result<String, String> {
    match kind {
        SystemActionKind::Lock => {
            log_info("Executing System Action: LockWorkStation");
            unsafe {
                let _ = windows::Win32::System::Shutdown::LockWorkStation();
            }
            Ok("Bilgisayar kilitlendi".into())
        }
        SystemActionKind::Sleep => {
            log_info("Executing System Action: Sleep");
            let _ = Command::new("rundll32.exe")
                .args(["powrprof.dll,SetSuspendState", "0,1,0"])
                .spawn();
            Ok("Uyku moduna geçildi".into())
        }
        SystemActionKind::Restart => {
            log_info("Executing System Action: Restart");
            let _ = Command::new("shutdown.exe").args(["/r", "/t", "0"]).spawn();
            Ok("Yeniden başlatılıyor...".into())
        }
        SystemActionKind::Shutdown => {
            log_info("Executing System Action: Shutdown");
            let _ = Command::new("shutdown.exe").args(["/s", "/t", "0"]).spawn();
            Ok("Bilgisayar kapatılıyor...".into())
        }
        SystemActionKind::EmptyRecycleBin => {
            log_info("Executing System Action: EmptyRecycleBin");
            unsafe {
                let flags = SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND;
                let _ = SHEmptyRecycleBinW(Some(HWND::default()), PCWSTR::null(), flags);
            }
            Ok("Geri Dönüşüm Kutusu boşaltıldı".into())
        }
        SystemActionKind::ToggleMute => {
            log_info("Executing System Action: Toggle Mute");
            unsafe {
                let _ = PostMessageW(
                    Some(HWND_BROADCAST),
                    WM_APPCOMMAND,
                    WPARAM(0),
                    LPARAM(APPCOMMAND_VOLUME_MUTE << 16),
                );
            }
            Ok("Ses durumu değiştirildi".into())
        }
        SystemActionKind::VolumeUp => {
            log_info("Executing System Action: Volume Up");
            unsafe {
                for _ in 0..3 {
                    let _ = PostMessageW(
                        Some(HWND_BROADCAST),
                        WM_APPCOMMAND,
                        WPARAM(0),
                        LPARAM(APPCOMMAND_VOLUME_UP << 16),
                    );
                }
            }
            Ok("Ses seviyesi artırıldı".into())
        }
        SystemActionKind::VolumeDown => {
            log_info("Executing System Action: Volume Down");
            unsafe {
                for _ in 0..3 {
                    let _ = PostMessageW(
                        Some(HWND_BROADCAST),
                        WM_APPCOMMAND,
                        WPARAM(0),
                        LPARAM(APPCOMMAND_VOLUME_DOWN << 16),
                    );
                }
            }
            Ok("Ses seviyesi azaltıldı".into())
        }
        SystemActionKind::OpenTaskMgr => {
            log_info("Executing System Action: Task Manager");
            let _ = Command::new("taskmgr.exe").spawn();
            Ok("Görev Yöneticisi açıldı".into())
        }
        SystemActionKind::ShowLocalIp => {
            let ip = get_local_ip_address();
            let _ = crate::clipboard::set_clipboard_text(&ip);
            Ok(format!("IP Kopyalandı: {}", ip))
        }
    }
}

pub fn get_local_ip_address() -> String {
    // Attempt fast local IP discovery via UDP socket trick without sending packets
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0")
        && socket.connect("8.8.8.8:80").is_ok()
        && let Ok(addr) = socket.local_addr()
    {
        return addr.ip().to_string();
    }
    "127.0.0.1".into()
}
