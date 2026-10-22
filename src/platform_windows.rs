use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::Mutex;
use windows::Win32::Foundation::{
    COLORREF, CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HINSTANCE, HWND, LPARAM,
    LRESULT, POINT, WPARAM,
};
use windows::Win32::Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::Win32::System::Threading::{AttachThreadInput, CreateMutexW, GetCurrentThreadId};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey,
    SetActiveWindow, SetFocus, UnregisterHotKey, VK_CONTROL, VK_F2, VK_K, VK_MENU, VK_SPACE,
};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW, Shell_NotifyIconW, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, BringWindowToTop, CallNextHookEx, CreatePopupMenu, DestroyMenu, FindWindowW,
    GWL_EXSTYLE, GetCursorPos, GetForegroundWindow, GetWindowLongW, GetWindowThreadProcessId,
    HHOOK, HICON, HWND_TOPMOST, ICON_BIG, ICON_SMALL, IDI_APPLICATION, IMAGE_FLAGS, IMAGE_ICON,
    KBDLLHOOKSTRUCT, LR_DEFAULTCOLOR, LWA_ALPHA, LoadIconW, LoadImageW, MF_SEPARATOR, MF_STRING,
    PostMessageW, SW_SHOWNORMAL, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SendMessageW,
    SetForegroundWindow, SetLayeredWindowAttributes, SetWindowLongW, SetWindowPos,
    SetWindowsHookExW, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenuEx,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SETICON, WM_SYSKEYDOWN, WS_EX_LAYERED,
};
use windows::core::PCWSTR;

use crate::config::AppConfig;
use crate::telemetry_local::{log_error, log_info};

pub const HOTKEY_ID: i32 = 1001;
pub const WM_TRAYICON: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 101;
pub const WM_TOGGLE_LAUNCHER: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 202;

pub const IDM_OPEN_LAUNCHER: u32 = 2001;
pub const IDM_OPEN_SETTINGS: u32 = 2002;
pub const IDM_REINDEX: u32 = 2003;
pub const IDM_QUIT: u32 = 2004;

const AUTOSTART_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const AUTOSTART_NAME: &str = "LiteCast";

static KEYBOARD_HOOK: Mutex<Option<isize>> = Mutex::new(None);
static HOOK_TARGET_HWND: Mutex<Option<isize>> = Mutex::new(None);

pub const EMBEDDED_ICON: &[u8] = include_bytes!("../assets/icon.ico");

pub fn enable_dpi_awareness() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

pub fn get_app_icon(size: i32) -> HICON {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(PCWSTR::null()).unwrap_or_default().0);
        // Try loading compiled PE resource icon (ID 1)
        if let Ok(handle) = LoadImageW(
            Some(hinstance),
            PCWSTR(std::ptr::dangling::<u16>()),
            IMAGE_ICON,
            size,
            size,
            LR_DEFAULTCOLOR,
        ) && !handle.0.is_null()
        {
            return HICON(handle.0);
        }

        // Fallback: create icon directly from embedded icon data
        let offset = windows::Win32::UI::WindowsAndMessaging::LookupIconIdFromDirectoryEx(
            EMBEDDED_ICON.as_ptr(),
            true,
            size,
            size,
            IMAGE_FLAGS(0),
        );
        if offset > 0
            && (offset as usize) < EMBEDDED_ICON.len()
            && let Ok(hi) = windows::Win32::UI::WindowsAndMessaging::CreateIconFromResourceEx(
                &EMBEDDED_ICON[offset as usize..],
                true,
                0x00030000,
                size,
                size,
                IMAGE_FLAGS(0),
            )
        {
            return hi;
        }

        LoadIconW(None, IDI_APPLICATION).unwrap_or_default()
    }
}

pub fn set_window_icons(hwnd: HWND) {
    unsafe {
        let icon_big = get_app_icon(64);
        let icon_small = get_app_icon(32);
        let _ = SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_BIG as usize)),
            Some(LPARAM(icon_big.0 as isize)),
        );
        let _ = SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_SMALL as usize)),
            Some(LPARAM(icon_small.0 as isize)),
        );
    }
}

pub fn set_window_opacity(hwnd: HWND, opacity_percent: u8) {
    unsafe {
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED.0 as i32);
        let alpha = ((opacity_percent.clamp(20, 100) as u32 * 255) / 100) as u8;
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
    }
}

pub fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub fn init_keyboard_hook(hwnd_launcher: HWND) {
    let mut target_guard = HOOK_TARGET_HWND.lock().unwrap();
    *target_guard = Some(hwnd_launcher.0 as isize);

    let mut hook_guard = KEYBOARD_HOOK.lock().unwrap();
    if hook_guard.is_none() {
        unsafe {
            let hinstance = HINSTANCE(GetModuleHandleW(PCWSTR::null()).unwrap_or_default().0);
            if let Ok(hook) = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(low_level_keyboard_proc),
                Some(hinstance),
                0,
            ) {
                *hook_guard = Some(hook.0 as isize);
                log_info("Installed low-level keyboard hook for rock-solid hotkeys.");
            } else {
                log_error("Failed to install low-level keyboard hook.");
            }
        }
    }
}

pub fn unhook_keyboard_hook() {
    let mut hook_guard = KEYBOARD_HOOK.lock().unwrap();
    if let Some(h) = hook_guard.take() {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(h as *mut _));
        }
    }
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 && (wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize) {
        unsafe {
            let kbd = *(lparam.0 as *const KBDLLHOOKSTRUCT);
            let cfg = AppConfig::load();

            let is_alt_down = (GetKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0;
            let is_ctrl_down = (GetKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;

            let triggered = match cfg.hotkey_index {
                0 => is_alt_down && kbd.vkCode == VK_SPACE.0 as u32,
                1 => is_ctrl_down && kbd.vkCode == VK_SPACE.0 as u32,
                2 => is_ctrl_down && kbd.vkCode == VK_K.0 as u32,
                3 => kbd.vkCode == VK_F2.0 as u32,
                _ => is_alt_down && kbd.vkCode == VK_SPACE.0 as u32,
            };

            if triggered {
                if let Some(hwnd_val) = *HOOK_TARGET_HWND.lock().unwrap() {
                    let target_hwnd = HWND(hwnd_val as *mut _);
                    let _ =
                        PostMessageW(Some(target_hwnd), WM_TOGGLE_LAUNCHER, WPARAM(0), LPARAM(0));
                }
                // Eat the keystroke so Windows system menu doesn't steal focus or pop up
                return LRESULT(1);
            }
        }
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

pub fn register_configured_hotkey(hwnd: HWND, hotkey_index: usize) -> bool {
    unsafe {
        let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);

        let (modifiers, vk) = match hotkey_index {
            0 => (MOD_ALT.0 | MOD_NOREPEAT.0, 0x20),     // Alt + Space
            1 => (MOD_CONTROL.0 | MOD_NOREPEAT.0, 0x20), // Ctrl + Space
            2 => (MOD_CONTROL.0 | MOD_NOREPEAT.0, 0x4B), // Ctrl + K
            3 => (0, 0x71),                              // F2
            _ => (MOD_ALT.0 | MOD_NOREPEAT.0, 0x20),
        };

        let res = RegisterHotKey(Some(hwnd), HOTKEY_ID, HOT_KEY_MODIFIERS(modifiers), vk);
        if res.is_err() && modifiers & MOD_NOREPEAT.0 != 0 {
            RegisterHotKey(
                Some(hwnd),
                HOTKEY_ID,
                HOT_KEY_MODIFIERS(modifiers & !MOD_NOREPEAT.0),
                vk,
            )
            .is_ok()
        } else {
            res.is_ok()
        }
    }
}

pub fn unregister_hotkey(hwnd: HWND) {
    unsafe {
        let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
    }
}

pub fn add_tray_icon(hwnd: HWND) -> bool {
    unsafe {
        let icon = get_app_icon(32);
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAYICON,
            hIcon: icon,
            szTip: [0; 128],
            ..Default::default()
        };

        let tip = to_wide("LiteCast Quick Launcher");
        for (i, &c) in tip.iter().take(127).enumerate() {
            nid.szTip[i] = c;
        }

        Shell_NotifyIconW(NIM_ADD, &nid).as_bool()
    }
}

pub fn remove_tray_icon(hwnd: HWND) {
    unsafe {
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

pub fn show_tray_notification(hwnd: HWND, title: &str, message: &str) {
    unsafe {
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_INFO,
            dwInfoFlags: NIIF_INFO,
            ..Default::default()
        };

        let wide_title = to_wide(title);
        for (i, &c) in wide_title.iter().take(63).enumerate() {
            nid.szInfoTitle[i] = c;
        }

        let wide_msg = to_wide(message);
        for (i, &c) in wide_msg.iter().take(255).enumerate() {
            nid.szInfo[i] = c;
        }

        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

pub fn show_tray_menu(hwnd: HWND) -> u32 {
    unsafe {
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);

        let hmenu = CreatePopupMenu().unwrap();
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            IDM_OPEN_LAUNCHER as usize,
            PCWSTR::from_raw(to_wide("🔍 Arama Çubuğunu Aç").as_ptr()),
        );
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            IDM_OPEN_SETTINGS as usize,
            PCWSTR::from_raw(to_wide("⚙️ Kontrol Paneli & Ayarlar").as_ptr()),
        );
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            IDM_QUIT as usize,
            PCWSTR::from_raw(to_wide("❌ LiteCast'ten Çıkış").as_ptr()),
        );

        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenuEx(
            hmenu,
            (TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN).0,
            pt.x,
            pt.y,
            hwnd,
            None,
        );
        let _ = DestroyMenu(hmenu);
        cmd.0 as u32
    }
}

pub fn center_window_on_active_monitor(hwnd: HWND, width: i32, height: i32) {
    unsafe {
        let mut pt = Default::default();
        let _ = GetCursorPos(&mut pt);

        let hmonitor: HMONITOR = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        if GetMonitorInfoW(hmonitor, &mut mi).as_bool() {
            let rc_work = mi.rcWork;
            let mon_width = rc_work.right - rc_work.left;
            let mon_height = rc_work.bottom - rc_work.top;

            let x = rc_work.left + (mon_width - width) / 2;
            let y = rc_work.top + (mon_height - height) / 3;

            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                width,
                height,
                SWP_SHOWWINDOW,
            );
        }
    }
}

pub fn force_foreground_window(hwnd: HWND) {
    unsafe {
        let foreground_hwnd = GetForegroundWindow();
        let foreground_thread = GetWindowThreadProcessId(foreground_hwnd, None);
        let current_thread = GetCurrentThreadId();

        let attached = if foreground_thread != 0 && foreground_thread != current_thread {
            AttachThreadInput(foreground_thread, current_thread, true).as_bool()
        } else {
            false
        };

        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetActiveWindow(hwnd);
        let _ = SetFocus(Some(hwnd));

        if attached {
            let _ = AttachThreadInput(foreground_thread, current_thread, false);
        }
    }
}

pub fn launch_target(target: &str, args: Option<&str>) -> Result<(), String> {
    let wide_target = to_wide(target);
    let wide_open = to_wide("open");
    let wide_args = args.map(to_wide);

    let result = unsafe {
        ShellExecuteW(
            Some(HWND::default()),
            PCWSTR::from_raw(wide_open.as_ptr()),
            PCWSTR::from_raw(wide_target.as_ptr()),
            if let Some(ref a) = wide_args {
                PCWSTR::from_raw(a.as_ptr())
            } else {
                PCWSTR::null()
            },
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };

    if (result.0 as usize) > 32 {
        Ok(())
    } else {
        // Robust fallback using explorer.exe (especially for shell:AppsFolder\..., uwp, and custom protocols)
        let mut cmd = std::process::Command::new("explorer.exe");
        cmd.arg(target);
        if let Some(a) = args {
            cmd.arg(a);
        }
        match cmd.spawn() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!(
                "ShellExecuteW failed ({}) and explorer fallback failed: {}",
                result.0 as usize, e
            )),
        }
    }
}

pub fn launch_target_elevated(
    target: &str,
    args: Option<&str>,
    working_dir: Option<&str>,
) -> Result<(), String> {
    let wide_target = to_wide(target);
    let wide_runas = to_wide("runas");
    let wide_args = args.map(to_wide);
    let wide_dir = working_dir.map(to_wide);

    let result = unsafe {
        ShellExecuteW(
            Some(HWND::default()),
            PCWSTR::from_raw(wide_runas.as_ptr()),
            PCWSTR::from_raw(wide_target.as_ptr()),
            if let Some(ref a) = wide_args {
                PCWSTR::from_raw(a.as_ptr())
            } else {
                PCWSTR::null()
            },
            if let Some(ref d) = wide_dir {
                PCWSTR::from_raw(d.as_ptr())
            } else {
                PCWSTR::null()
            },
            SW_SHOWNORMAL,
        )
    };

    if (result.0 as usize) > 32 {
        Ok(())
    } else {
        // Fallback using PowerShell Start-Process -Verb RunAs
        let mut ps_cmd = format!("Start-Process -FilePath \"{}\"", target);
        if let Some(a) = args {
            ps_cmd.push_str(&format!(" -ArgumentList \"{}\"", a));
        }
        if let Some(d) = working_dir {
            ps_cmd.push_str(&format!(" -WorkingDirectory \"{}\"", d));
        }
        ps_cmd.push_str(" -Verb RunAs");

        let status = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &ps_cmd])
            .status();

        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => Err(format!(
                "ShellExecuteW (runas) code {} and PowerShell RunAs exited with code {:?}",
                result.0 as usize,
                s.code()
            )),
            Err(e) => Err(format!(
                "ShellExecuteW (runas) code {} and PowerShell spawn failed: {}",
                result.0 as usize, e
            )),
        }
    }
}

pub fn is_autostart_enabled() -> bool {
    let wide_subkey = to_wide(AUTOSTART_SUBKEY);
    let wide_name = to_wide(AUTOSTART_NAME);
    let mut hkey = Default::default();

    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(wide_subkey.as_ptr()),
            Some(0),
            KEY_QUERY_VALUE,
            &mut hkey,
        )
        .is_ok()
        {
            let res = RegQueryValueExW(
                hkey,
                PCWSTR::from_raw(wide_name.as_ptr()),
                None,
                None,
                None,
                None,
            )
            .is_ok();
            let _ = RegCloseKey(hkey);
            res
        } else {
            false
        }
    }
}

pub fn set_autostart(enable: bool) -> Result<(), String> {
    let wide_subkey = to_wide(AUTOSTART_SUBKEY);
    let wide_name = to_wide(AUTOSTART_NAME);
    let mut hkey = Default::default();

    unsafe {
        let open_res = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(wide_subkey.as_ptr()),
            Some(0),
            KEY_SET_VALUE | KEY_QUERY_VALUE,
            &mut hkey,
        );
        if open_res.is_err() {
            return Err(format!(
                "Failed to open Registry Run key (error {})",
                open_res.0
            ));
        }

        let result = if enable {
            let current_exe = std::env::current_exe()
                .map_err(|e| format!("Cannot get current exe path: {}", e))?;
            let exe_str = current_exe.to_string_lossy().to_string();
            let wide_val = to_wide(&format!("\"{}\"", exe_str));
            let bytes = std::slice::from_raw_parts(
                wide_val.as_ptr() as *const u8,
                wide_val.len() * std::mem::size_of::<u16>(),
            );

            let set_res = RegSetValueExW(
                hkey,
                PCWSTR::from_raw(wide_name.as_ptr()),
                Some(0),
                REG_SZ,
                Some(bytes),
            );
            if set_res.is_ok() {
                Ok(())
            } else {
                Err(format!("RegSetValueExW failed with error {}", set_res.0))
            }
        } else {
            let del_res = RegDeleteValueW(hkey, PCWSTR::from_raw(wide_name.as_ptr()));
            if del_res.is_ok() {
                Ok(())
            } else {
                Err(format!("RegDeleteValueW failed with error {}", del_res.0))
            }
        };

        let _ = RegCloseKey(hkey);
        result
    }
}

pub struct SingleInstanceGuard {
    handle: HANDLE,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if !self.handle.0.is_null() {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

pub fn try_acquire_single_instance() -> Option<SingleInstanceGuard> {
    unsafe {
        let name = to_wide("Local\\LiteCast_SingleInstance_Mutex_v1");
        let handle = CreateMutexW(None, true, PCWSTR::from_raw(name.as_ptr()));

        if let Ok(h) = handle {
            if GetLastError() == ERROR_ALREADY_EXISTS {
                // Another instance is already running. Signal the existing window to toggle/foreground.
                let class_name = to_wide("LiteCastWindowClass");
                if let Ok(hwnd) = FindWindowW(PCWSTR::from_raw(class_name.as_ptr()), PCWSTR::null())
                    && !hwnd.0.is_null()
                {
                    force_foreground_window(hwnd);
                    let _ = PostMessageW(Some(hwnd), WM_TOGGLE_LAUNCHER, WPARAM(0), LPARAM(0));
                }
                let _ = CloseHandle(h);
                return None;
            }
            Some(SingleInstanceGuard { handle: h })
        } else {
            None
        }
    }
}

pub fn apply_modern_window_styling(hwnd: HWND) {
    unsafe {
        // 1. Dark Mode Frame (DWMWA_USE_IMMERSIVE_DARK_MODE = 20)
        let dark_mode: i32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(20),
            &dark_mode as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );

        // 2. Rounded Corners on Windows 11 (DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWCP_ROUND = 2)
        let corner_pref: i32 = 2;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(33),
            &corner_pref as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );

        // 3. Mica / Acrylic Backdrop on Windows 11 Build 22000+ (DWMWA_SYSTEMBACKDROP_TYPE = 38, DWMSBT_TRANSIENTWINDOW = 3)
        let backdrop_type: i32 = 3; // Acrylic / Transient
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(38),
            &backdrop_type as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );
    }
}
