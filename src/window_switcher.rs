// Active Window Enumeration & Fast Switching for LiteCast.

use std::sync::Mutex;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW,
    IsWindowVisible, SW_RESTORE, SetForegroundWindow, ShowWindow, WS_EX_TOOLWINDOW,
};
use windows::core::BOOL;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveWindowItem {
    pub hwnd_val: isize,
    pub title: String,
    pub preview: String,
    pub badge: &'static str,
}

static COLLECTED_WINDOWS: Mutex<Vec<ActiveWindowItem>> = Mutex::new(Vec::new());

pub fn get_open_windows() -> Vec<ActiveWindowItem> {
    {
        let mut guard = COLLECTED_WINDOWS.lock().unwrap();
        guard.clear();
    }

    unsafe {
        let _ = EnumWindows(Some(enum_windows_proc), LPARAM(0));
    }

    let guard = COLLECTED_WINDOWS.lock().unwrap();
    guard.clone()
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }

        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        if (ex_style & WS_EX_TOOLWINDOW.0 as i32) != 0 {
            return BOOL(1);
        }

        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 || len > 500 {
            return BOOL(1);
        }

        let mut buf = vec![0u16; (len + 1) as usize];
        let read_len = GetWindowTextW(hwnd, &mut buf);
        if read_len <= 0 {
            return BOOL(1);
        }

        let title = String::from_utf16_lossy(&buf[..read_len as usize])
            .trim()
            .to_string();
        if title.is_empty() {
            return BOOL(1);
        }

        // Filter out LiteCast windows and special Windows internal shells
        if title.starts_with("LiteCast")
            || title == "Program Manager"
            || title == "Settings"
            || title == "Windows Input Experience"
        {
            return BOOL(1);
        }

        let mut guard = COLLECTED_WINDOWS.lock().unwrap();
        guard.push(ActiveWindowItem {
            hwnd_val: hwnd.0 as isize,
            title: format!("🪟 {}", title),
            preview: "Açık pencereye odaklanmak için Enter'a basın".to_string(),
            badge: "Pencere",
        });

        BOOL(1)
    }
}

pub fn search_open_windows(query: &str) -> Vec<ActiveWindowItem> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }

    let is_explicit = q.starts_with("win ")
        || q.starts_with("tab ")
        || q.starts_with("w:")
        || q == "win"
        || q == "tab"
        || q == "windows";

    let clean_query = if let Some(rest) = q.strip_prefix("win ") {
        rest.trim()
    } else if let Some(rest) = q.strip_prefix("tab ") {
        rest.trim()
    } else if let Some(rest) = q.strip_prefix("w:") {
        rest.trim()
    } else if q == "win" || q == "tab" || q == "windows" {
        ""
    } else {
        &q
    };

    let all_windows = get_open_windows();

    if is_explicit && clean_query.is_empty() {
        return all_windows;
    }

    all_windows
        .into_iter()
        .filter(|w| {
            if is_explicit {
                w.title.to_lowercase().contains(clean_query)
            } else {
                // Only match if query is at least 3 chars long and strongly matches window title
                clean_query.len() >= 3 && w.title.to_lowercase().contains(clean_query)
            }
        })
        .collect()
}

pub fn focus_window(hwnd_val: isize) -> bool {
    let hwnd = HWND(hwnd_val as *mut _);
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd).as_bool()
    }
}
