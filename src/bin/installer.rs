#![windows_subsystem = "windows"]
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_match,
    clippy::too_many_arguments,
    clippy::unnecessary_map_or
)]

// LiteCast Kurulum, Doğrulama & Bakım Sihirbazı (Installer & Verifier Wizard)
// Windows 10/11 için yerel, ultra hızlı, minimalist koyu temalı Win32 kurulum yöneticisi.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreateSolidBrush,
    DRAW_TEXT_FORMAT, DT_CENTER, DT_LEFT, DT_NOPREFIX, DT_PATH_ELLIPSIS, DT_SINGLELINE, DT_VCENTER,
    DT_WORDBREAK, DeleteDC, DeleteObject, DrawTextW, EndPaint, FONT_CHARSET, FONT_CLIP_PRECISION,
    FONT_OUTPUT_PRECISION, FONT_QUALITY, FillRect, HDC, HFONT, InvalidateRect, PAINTSTRUCT,
    SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize, IPersistFile,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegCreateKeyW,
    RegDeleteKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::Shell::{
    FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog, IShellLinkW,
    SIGDN_FILESYSPATH, ShellLink,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetClientRect, GetMessageW, GetSystemMetrics, HTCAPTION, IDC_ARROW, IDC_HAND, LoadCursorW, MSG,
    PostMessageW, PostQuitMessage, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_MINIMIZE, SW_SHOW,
    SWP_NOZORDER, SWP_SHOWWINDOW, SendMessageW, SetCursor, SetForegroundWindow, SetWindowPos,
    ShowWindow, TranslateMessage, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_MOUSEMOVE, WM_NCLBUTTONDOWN, WM_PAINT, WNDCLASSW, WS_EX_APPWINDOW, WS_POPUP, WS_VISIBLE,
};
use windows::core::{Interface, PCWSTR};

// Gömülü LiteCast binary'si (Dağıtım için standalone payload)
const EMBEDDED_BINARY: &[u8] = include_bytes!("../../LiteCast.exe");

const APP_NAME: &str = "LiteCast";
const APP_VERSION: &str = "1.0.1";
const PUBLISHER: &str = "LiteCast Team";
const RUN_REG_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const UNINSTALL_REG_PATH: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\LiteCast";

const WND_WIDTH: i32 = 670;
const WND_HEIGHT: i32 = 480;

const WM_APP_ACTION_DONE: u32 = 0x8000 + 1;
const WM_APP_PROGRESS_UPDATE: u32 = 0x8000 + 2;

// ---------------- Color System (Modern Minimal Dark) ----------------

#[inline]
pub const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | ((g as u32) << 8) | ((b as u32) << 16))
}

const COLOR_BG: COLORREF = rgb(18, 19, 24);
const COLOR_CARD: COLORREF = rgb(25, 26, 34);
const COLOR_CARD_HOVER: COLORREF = rgb(33, 35, 46);
const COLOR_CARD_INNER: COLORREF = rgb(15, 16, 21);
const COLOR_BORDER: COLORREF = rgb(45, 47, 62);
const COLOR_BORDER_SUBTLE: COLORREF = rgb(35, 37, 49);

const COLOR_ACCENT: COLORREF = rgb(37, 99, 235); // #2563eb
const COLOR_ACCENT_HOVER: COLORREF = rgb(59, 130, 246); // #3b82f6
const COLOR_ACCENT_BORDER: COLORREF = rgb(96, 165, 250); // #60a5fa

const COLOR_SUCCESS_TEXT: COLORREF = rgb(74, 222, 128); // #4ade80
const COLOR_WARNING_TEXT: COLORREF = rgb(251, 191, 36); // #fbbf24

const COLOR_DANGER_BG: COLORREF = rgb(32, 18, 22);
const COLOR_DANGER_HOVER_BG: COLORREF = rgb(50, 22, 28);
const COLOR_DANGER_BORDER: COLORREF = rgb(90, 28, 34);
const COLOR_DANGER_TEXT: COLORREF = rgb(248, 113, 113); // #f87171

const COLOR_TEXT_PRIMARY: COLORREF = rgb(244, 244, 245);
const COLOR_TEXT_SECONDARY: COLORREF = rgb(156, 163, 175);
const COLOR_TEXT_DIM: COLORREF = rgb(113, 113, 122);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WizardPage {
    Welcome,
    Options,
    Processing {
        action_name: String,
        progress: usize,
        logs: Vec<String>,
    },
    Completed {
        summary: String,
        is_repair: bool,
    },
    Uninstalled,
}

#[derive(Debug, Clone)]
pub struct InstallationState {
    pub is_already_installed: bool,
    pub installed_path: PathBuf,
    pub installed_size: u64,
    pub installed_hash: String,
    pub expected_hash: String,
    pub is_integrity_valid: bool,
    pub autostart_enabled: bool,
    pub desktop_shortcut_exists: bool,
    pub startmenu_shortcut_exists: bool,
}

#[derive(Debug, Clone)]
pub struct WizardState {
    pub page: WizardPage,
    pub install_dir: PathBuf,
    pub opt_autostart: bool,
    pub opt_desktop_shortcut: bool,
    pub opt_startmenu_shortcut: bool,
    pub opt_launch_after: bool,
    pub installation_info: InstallationState,
    pub hovered_btn: Option<usize>,
}

static STATE: Mutex<Option<WizardState>> = Mutex::new(None);

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn draw_text(
    hdc: HDC,
    text: &str,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    flags: DRAW_TEXT_FORMAT,
) {
    let mut wide = to_wide(text);
    let mut rect = RECT {
        left,
        top,
        right,
        bottom,
    };
    unsafe {
        let len = wide.len().saturating_sub(1);
        let _ = DrawTextW(hdc, &mut wide[..len], &mut rect, flags | DT_NOPREFIX);
    }
}

fn create_font(size: i32, weight: i32) -> HFONT {
    unsafe {
        let font_name = to_wide("Segoe UI");
        CreateFontW(
            -size,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            FONT_CHARSET(1),
            FONT_OUTPUT_PRECISION(0),
            FONT_CLIP_PRECISION(0),
            FONT_QUALITY(5),
            0,
            PCWSTR::from_raw(font_name.as_ptr()),
        )
    }
}

fn get_default_install_dir() -> PathBuf {
    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local_appdata).join("Programs").join(APP_NAME)
    } else {
        PathBuf::from(r"C:\Program Files").join(APP_NAME)
    }
}

fn get_desktop_shortcut_path() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Public".into());
    PathBuf::from(home)
        .join("Desktop")
        .join(format!("{}.lnk", APP_NAME))
}

fn get_start_menu_shortcut_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join(format!("{}.lnk", APP_NAME))
    } else {
        get_desktop_shortcut_path()
    }
}

// ---------------- Integrity & SHA-256 ----------------

fn compute_sha256(data: &[u8]) -> String {
    litecast::dev_tools::sha256_hex(data)
}

fn detect_system_state(install_dir: &Path) -> InstallationState {
    let binary_path = install_dir.join("LiteCast.exe");

    let expected_hash = compute_sha256(EMBEDDED_BINARY);
    let mut is_already_installed = false;
    let mut installed_size = 0;
    let mut installed_hash = String::new();
    let mut is_integrity_valid = false;

    if binary_path.exists() {
        is_already_installed = true;
        if let Ok(metadata) = fs::metadata(&binary_path) {
            installed_size = metadata.len();
        }
        if let Ok(bytes) = fs::read(&binary_path) {
            installed_hash = compute_sha256(&bytes);
            is_integrity_valid = installed_hash == expected_hash;
        }
    }

    let autostart_enabled = is_registry_autostart_enabled(&binary_path);
    let desktop_shortcut_exists = get_desktop_shortcut_path().exists();
    let startmenu_shortcut_exists = get_start_menu_shortcut_path().exists();

    InstallationState {
        is_already_installed,
        installed_path: binary_path,
        installed_size,
        installed_hash,
        expected_hash,
        is_integrity_valid,
        autostart_enabled,
        desktop_shortcut_exists,
        startmenu_shortcut_exists,
    }
}

fn pick_install_directory(hwnd: HWND, _initial_dir: &Path) -> Option<PathBuf> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let dialog_res: windows::core::Result<IFileOpenDialog> =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL);

        if let Ok(dialog) = dialog_res {
            let _ = dialog.SetOptions(FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM);
            let title_w = to_wide("LiteCast Kurulum Klasörünü Seçin");
            let _ = dialog.SetTitle(PCWSTR::from_raw(title_w.as_ptr()));

            if dialog.Show(Some(hwnd)).is_ok() {
                if let Ok(item) = dialog.GetResult() {
                    if let Ok(path_ptr) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                        let path_slice = std::slice::from_raw_parts(path_ptr.0, {
                            let mut len = 0;
                            while *path_ptr.0.add(len) != 0 {
                                len += 1;
                            }
                            len
                        });
                        let selected = String::from_utf16_lossy(path_slice);
                        CoTaskMemFree(Some(path_ptr.0 as *const _));
                        CoUninitialize();

                        let mut p = PathBuf::from(selected);
                        if p.file_name().map_or(false, |n| {
                            n.to_string_lossy().eq_ignore_ascii_case("LiteCast.exe")
                        }) {
                            p.pop();
                        }
                        return Some(p);
                    }
                }
            }
        }
        CoUninitialize();
    }
    None
}

// ---------------- Windows Registry Helpers ----------------

fn get_registered_install_dir() -> Option<PathBuf> {
    let wide_sub = to_wide(UNINSTALL_REG_PATH);
    let wide_val_name = to_wide("InstallLocation");
    let mut hkey = Default::default();

    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(wide_sub.as_ptr()),
            Some(0),
            KEY_QUERY_VALUE,
            &mut hkey,
        )
        .is_ok()
        {
            let mut buf = [0u16; 512];
            let mut buf_len = (buf.len() * std::mem::size_of::<u16>()) as u32;
            let res = RegQueryValueExW(
                hkey,
                PCWSTR::from_raw(wide_val_name.as_ptr()),
                None,
                None,
                Some(buf.as_mut_ptr() as *mut u8),
                Some(&mut buf_len),
            );
            let _ = RegCloseKey(hkey);
            if res.is_ok() {
                let char_len = (buf_len as usize / std::mem::size_of::<u16>()).min(buf.len());
                let slice = &buf[..char_len];
                let null_pos = slice.iter().position(|&c| c == 0).unwrap_or(char_len);
                let val = String::from_utf16_lossy(&slice[..null_pos])
                    .trim()
                    .to_string();
                if !val.is_empty() {
                    return Some(PathBuf::from(val));
                }
            }
        }
    }
    None
}

fn is_registry_autostart_enabled(binary_path: &Path) -> bool {
    let wide_sub = to_wide(RUN_REG_PATH);
    let wide_name = to_wide(APP_NAME);
    let mut hkey = Default::default();

    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(wide_sub.as_ptr()),
            Some(0),
            KEY_QUERY_VALUE,
            &mut hkey,
        )
        .is_ok()
        {
            let mut buf = [0u16; 512];
            let mut buf_len = (buf.len() * std::mem::size_of::<u16>()) as u32;
            let res = RegQueryValueExW(
                hkey,
                PCWSTR::from_raw(wide_name.as_ptr()),
                None,
                None,
                Some(buf.as_mut_ptr() as *mut u8),
                Some(&mut buf_len),
            );
            let _ = RegCloseKey(hkey);
            if res.is_ok() {
                let char_len = (buf_len as usize / std::mem::size_of::<u16>()).min(buf.len());
                let slice = &buf[..char_len];
                let null_pos = slice.iter().position(|&c| c == 0).unwrap_or(char_len);
                let val = String::from_utf16_lossy(&slice[..null_pos]);
                let bin_str = binary_path.to_string_lossy();
                return val.contains(bin_str.trim_matches('"'));
            }
        }
    }
    false
}

fn set_registry_autostart(binary_path: &Path, enable: bool) -> Result<(), String> {
    let wide_sub = to_wide(RUN_REG_PATH);
    let wide_name = to_wide(APP_NAME);
    let mut hkey = Default::default();

    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(wide_sub.as_ptr()),
            Some(0),
            KEY_SET_VALUE,
            &mut hkey,
        )
        .is_ok()
        {
            let res = if enable {
                let cmd_str = format!("\"{}\"", binary_path.display());
                let wide_val = to_wide(&cmd_str);
                RegSetValueExW(
                    hkey,
                    PCWSTR::from_raw(wide_name.as_ptr()),
                    None,
                    REG_SZ,
                    Some(std::slice::from_raw_parts(
                        wide_val.as_ptr() as *const u8,
                        wide_val.len() * 2,
                    )),
                )
            } else {
                RegDeleteValueW(hkey, PCWSTR::from_raw(wide_name.as_ptr()))
            };
            let _ = RegCloseKey(hkey);
            if res.is_ok() {
                Ok(())
            } else {
                Err(format!(
                    "Autostart registry işlemi başarısız (kod: {})",
                    res.0
                ))
            }
        } else {
            Err("Run registry key açılamadı".into())
        }
    }
}

fn register_uninstall_entry(install_dir: &Path, binary_path: &Path) -> Result<(), String> {
    let wide_sub = to_wide(UNINSTALL_REG_PATH);
    let mut hkey = Default::default();

    unsafe {
        if RegCreateKeyW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(wide_sub.as_ptr()),
            &mut hkey,
        )
        .is_ok()
        {
            let set_str = |key_name: &str, val_str: &str| -> Result<(), String> {
                let w_key = to_wide(key_name);
                let w_val = to_wide(val_str);
                let res = RegSetValueExW(
                    hkey,
                    PCWSTR::from_raw(w_key.as_ptr()),
                    None,
                    REG_SZ,
                    Some(std::slice::from_raw_parts(
                        w_val.as_ptr() as *const u8,
                        w_val.len() * 2,
                    )),
                );
                if res.is_ok() {
                    Ok(())
                } else {
                    Err(format!("Değer yazılamadı: {} (kod: {})", key_name, res.0))
                }
            };

            set_str("DisplayName", APP_NAME)?;
            set_str("DisplayVersion", APP_VERSION)?;
            set_str("Publisher", PUBLISHER)?;
            set_str("InstallLocation", &install_dir.to_string_lossy())?;
            set_str("DisplayIcon", &format!("\"{}\",0", binary_path.display()))?;

            let uninstaller_exe = install_dir.join("LiteCast-Setup.exe");
            set_str(
                "UninstallString",
                &format!("\"{}\" --uninstall", uninstaller_exe.display()),
            )?;

            let _ = RegCloseKey(hkey);
            Ok(())
        } else {
            Err("Uninstall registry kaydı oluşturulamadı".into())
        }
    }
}

fn unregister_uninstall_entry() {
    let wide_sub = to_wide(UNINSTALL_REG_PATH);
    unsafe {
        let _ = RegDeleteKeyW(HKEY_CURRENT_USER, PCWSTR::from_raw(wide_sub.as_ptr()));
    }
}

// ---------------- Native Instant COM Shortcut Creation ----------------

fn create_shortcut(
    target_exe: &Path,
    shortcut_path: &Path,
    description: &str,
) -> Result<(), String> {
    if let Some(parent) = shortcut_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let link_res: windows::core::Result<IShellLinkW> =
            CoCreateInstance(&ShellLink, None, CLSCTX_ALL);

        if let Ok(link) = link_res {
            let target_w = to_wide(&target_exe.to_string_lossy());
            let desc_w = to_wide(description);
            let dir_w = if let Some(parent) = target_exe.parent() {
                to_wide(&parent.to_string_lossy())
            } else {
                to_wide("")
            };

            let _ = link.SetPath(PCWSTR::from_raw(target_w.as_ptr()));
            let _ = link.SetDescription(PCWSTR::from_raw(desc_w.as_ptr()));
            let _ = link.SetWorkingDirectory(PCWSTR::from_raw(dir_w.as_ptr()));
            let _ = link.SetIconLocation(PCWSTR::from_raw(target_w.as_ptr()), 0);

            let persist_res: windows::core::Result<IPersistFile> = link.cast();
            if let Ok(persist) = persist_res {
                let shortcut_w = to_wide(&shortcut_path.to_string_lossy());
                let save_res = persist.Save(PCWSTR::from_raw(shortcut_w.as_ptr()), true);
                CoUninitialize();
                if save_res.is_ok() {
                    return Ok(());
                } else {
                    return Err(format!("Kısayol kaydedilemedi: {:?}", save_res));
                }
            }
        }
        CoUninitialize();
    }
    Err("ShellLink COM nesnesi oluşturulamadı".into())
}

// ---------------- Native Instant Process Termination ----------------

fn stop_running_litecast() {
    unsafe {
        // 1. Try graceful exit first by signaling the main launcher window
        let class_name = to_wide("LiteCastWindowClass");
        if let Ok(hwnd) = windows::Win32::UI::WindowsAndMessaging::FindWindowW(
            PCWSTR::from_raw(class_name.as_ptr()),
            PCWSTR::null(),
        ) && !hwnd.0.is_null()
        {
            let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(hwnd),
                windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                WPARAM(0),
                LPARAM(0),
            );
            std::thread::sleep(std::time::Duration::from_millis(250));
        }

        // 2. Terminate any remaining LiteCast process if still running
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if let Ok(snap) = snapshot {
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };

            if Process32FirstW(snap, &mut entry).is_ok() {
                loop {
                    let exe_name = String::from_utf16_lossy(&entry.szExeFile)
                        .trim_matches('\0')
                        .to_string();

                    if exe_name.eq_ignore_ascii_case("LiteCast.exe")
                        || exe_name.eq_ignore_ascii_case("litecast.exe")
                    {
                        if let Ok(proc_handle) =
                            OpenProcess(PROCESS_TERMINATE, false, entry.th32ProcessID)
                        {
                            let _ = TerminateProcess(proc_handle, 0);
                            let _ = windows::Win32::Foundation::CloseHandle(proc_handle);
                        }
                    }

                    if Process32NextW(snap, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = windows::Win32::Foundation::CloseHandle(snap);
        }
    }
}

fn start_installed_litecast(binary_path: &Path) {
    let _ = Command::new(binary_path).spawn();
}

// ---------------- Async Non-Blocking Workers ----------------

fn run_async_install_or_repair(hwnd: HWND, is_repair: bool) {
    let state_snapshot = {
        let guard = STATE.lock().unwrap();
        match guard.as_ref() {
            Some(s) => s.clone(),
            None => return,
        }
    };

    let action_title = if is_repair { "Onarım" } else { "Kurulum" };

    // Set initial processing state
    {
        let mut guard = STATE.lock().unwrap();
        if let Some(state) = guard.as_mut() {
            state.page = WizardPage::Processing {
                action_name: action_title.to_string(),
                progress: 15,
                logs: vec!["İşlem başlatılıyor...".to_string()],
            };
        }
    }
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, false);
    }

    let hwnd_raw = hwnd.0 as usize;

    std::thread::spawn(move || {
        let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
        let post_progress = |percent: usize, log: &str| {
            let mut guard = STATE.lock().unwrap();
            if let Some(state) = guard.as_mut() {
                if let WizardPage::Processing { progress, logs, .. } = &mut state.page {
                    *progress = percent;
                    logs.push(log.to_string());
                }
            }
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_APP_PROGRESS_UPDATE, WPARAM(0), LPARAM(0));
            }
            std::thread::sleep(std::time::Duration::from_millis(60));
        };

        // 1. Process termination
        post_progress(30, "Çalışan LiteCast süreçleri durduruluyor...");
        stop_running_litecast();

        // 2. Directory creation & Payload write
        let install_dir = state_snapshot.install_dir;
        let target_binary = install_dir.join("LiteCast.exe");
        let setup_binary_in_dir = install_dir.join("LiteCast-Setup.exe");

        post_progress(
            50,
            "Hedef klasör hazırlanıyor ve program dosyaları kopyalanıyor...",
        );
        if let Err(e) = fs::create_dir_all(&install_dir) {
            finish_with_result(
                hwnd,
                Err(format!("Klasör oluşturulamadı: {}", e)),
                is_repair,
            );
            return;
        }

        if let Err(e) = fs::write(&target_binary, EMBEDDED_BINARY) {
            finish_with_result(hwnd, Err(format!("Dosya yazılamadı: {}", e)), is_repair);
            return;
        }

        if let Ok(current_exe) = std::env::current_exe() {
            let _ = fs::copy(current_exe, &setup_binary_in_dir);
        }

        // 3. SHA-256 Verification
        post_progress(70, "Dosya bütünlüğü SHA-256 ile doğrulanıyor...");
        let written_bytes = match fs::read(&target_binary) {
            Ok(b) => b,
            Err(e) => {
                finish_with_result(
                    hwnd,
                    Err(format!("Doğrulama okuma hatası: {}", e)),
                    is_repair,
                );
                return;
            }
        };

        let computed_hash = compute_sha256(&written_bytes);
        let expected_hash = compute_sha256(EMBEDDED_BINARY);
        if computed_hash != expected_hash {
            finish_with_result(
                hwnd,
                Err("SHA-256 bütünlük doğrulaması başarısız!".into()),
                is_repair,
            );
            return;
        }

        // 4. Registry & Shortcuts
        post_progress(85, "Sistem başlangıcı ve kısayollar yapılandırılıyor...");
        let _ = set_registry_autostart(&target_binary, state_snapshot.opt_autostart);
        let _ = register_uninstall_entry(&install_dir, &target_binary);

        if state_snapshot.opt_desktop_shortcut {
            let _ = create_shortcut(
                &target_binary,
                &get_desktop_shortcut_path(),
                "LiteCast Hızlı Başlatıcı & Arama",
            );
        }

        if state_snapshot.opt_startmenu_shortcut {
            let _ = create_shortcut(
                &target_binary,
                &get_start_menu_shortcut_path(),
                "LiteCast Hızlı Başlatıcı & Arama",
            );
        }

        post_progress(100, "Tüm işlemler başarıyla tamamlandı!");
        std::thread::sleep(std::time::Duration::from_millis(80));

        let msg = format!(
            "LiteCast başarıyla yüklendi ve doğrulandı!\n\n• Kurulum Konumu: {}\n• Dosya Boyutu: {:.2} MB (Bütünlük Tam)",
            target_binary.display(),
            (written_bytes.len() as f64) / (1024.0 * 1024.0)
        );

        finish_with_result(hwnd, Ok(msg), is_repair);
    });
}

fn finish_with_result(hwnd: HWND, res: Result<String, String>, is_repair: bool) {
    let mut guard = STATE.lock().unwrap();
    if let Some(state) = guard.as_mut() {
        match res {
            Ok(msg) => {
                state.page = WizardPage::Completed {
                    summary: msg,
                    is_repair,
                };
            }
            Err(e) => {
                state.page = WizardPage::Completed {
                    summary: format!("Hata: {}", e),
                    is_repair,
                };
            }
        }
    }
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_APP_ACTION_DONE, WPARAM(0), LPARAM(0));
    }
}

fn run_async_uninstall(hwnd: HWND) {
    let install_dir = {
        let guard = STATE.lock().unwrap();
        guard
            .as_ref()
            .map(|s| s.install_dir.clone())
            .or_else(get_registered_install_dir)
            .unwrap_or_else(get_default_install_dir)
    };

    {
        let mut guard = STATE.lock().unwrap();
        if let Some(state) = guard.as_mut() {
            state.page = WizardPage::Processing {
                action_name: "Kaldırma".to_string(),
                progress: 25,
                logs: vec!["LiteCast sistemden kaldırılıyor...".to_string()],
            };
        }
    }
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, false);
    }

    let hwnd_raw = hwnd.0 as usize;

    std::thread::spawn(move || {
        let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
        stop_running_litecast();

        let target_binary = install_dir.join("LiteCast.exe");
        let _ = set_registry_autostart(&target_binary, false);
        unregister_uninstall_entry();

        let desktop_lnk = get_desktop_shortcut_path();
        if desktop_lnk.exists() {
            let _ = fs::remove_file(desktop_lnk);
        }

        let start_lnk = get_start_menu_shortcut_path();
        if start_lnk.exists() {
            let _ = fs::remove_file(start_lnk);
        }

        if install_dir.exists() {
            let _ = fs::remove_dir_all(&install_dir);
        }

        let mut guard = STATE.lock().unwrap();
        if let Some(state) = guard.as_mut() {
            state.page = WizardPage::Uninstalled;
        }

        unsafe {
            let _ = PostMessageW(Some(hwnd), WM_APP_ACTION_DONE, WPARAM(0), LPARAM(0));
        }
    });
}

// ---------------- Modern Win32 Drawing Helpers ----------------

unsafe fn draw_rect_solid(hdc: HDC, rect: &RECT, bg_color: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(bg_color);
        FillRect(hdc, rect, brush);
        let _ = DeleteObject(brush.into());
    }
}

unsafe fn draw_card_box(hdc: HDC, rect: &RECT, bg_color: COLORREF, border_color: COLORREF) {
    unsafe {
        draw_rect_solid(hdc, rect, border_color);
        let inner = RECT {
            left: rect.left + 1,
            top: rect.top + 1,
            right: rect.right - 1,
            bottom: rect.bottom - 1,
        };
        draw_rect_solid(hdc, &inner, bg_color);
    }
}

unsafe fn draw_button(
    hdc: HDC,
    rect: RECT,
    text: &str,
    is_hovered: bool,
    is_primary: bool,
    is_danger: bool,
    font: HFONT,
) {
    let (bg, border, text_col) = if is_danger {
        if is_hovered {
            (COLOR_DANGER_HOVER_BG, rgb(185, 28, 28), rgb(254, 202, 202))
        } else {
            (COLOR_DANGER_BG, COLOR_DANGER_BORDER, COLOR_DANGER_TEXT)
        }
    } else if is_primary {
        if is_hovered {
            (COLOR_ACCENT_HOVER, COLOR_ACCENT_BORDER, rgb(255, 255, 255))
        } else {
            (COLOR_ACCENT, COLOR_ACCENT, rgb(255, 255, 255))
        }
    } else if is_hovered {
        (COLOR_CARD_HOVER, rgb(75, 78, 98), COLOR_TEXT_PRIMARY)
    } else {
        (COLOR_CARD, COLOR_BORDER, COLOR_TEXT_PRIMARY)
    };

    unsafe {
        draw_card_box(hdc, &rect, bg, border);
        let old_font = SelectObject(hdc, font.into());
        SetTextColor(hdc, text_col);
        draw_text(
            hdc,
            text,
            rect.left + 6,
            rect.top,
            rect.right - 6,
            rect.bottom,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        SelectObject(hdc, old_font);
    }
}

unsafe fn draw_checkbox_row(
    hdc: HDC,
    rect: RECT,
    checked: bool,
    label: &str,
    description: &str,
    is_hovered: bool,
    font_bold: HFONT,
    font_small: HFONT,
) {
    let card_bg = if is_hovered {
        COLOR_CARD_HOVER
    } else {
        COLOR_CARD
    };
    let card_border = if checked {
        COLOR_ACCENT
    } else if is_hovered {
        rgb(65, 68, 88)
    } else {
        COLOR_BORDER
    };

    unsafe {
        draw_card_box(hdc, &rect, card_bg, card_border);

        // Checkbox square
        let box_y = rect.top + (rect.bottom - rect.top - 18) / 2;
        let box_rect = RECT {
            left: rect.left + 14,
            top: box_y,
            right: rect.left + 32,
            bottom: box_y + 18,
        };

        let check_bg = if checked {
            COLOR_ACCENT
        } else {
            COLOR_CARD_INNER
        };
        let check_border = if checked { COLOR_ACCENT } else { COLOR_BORDER };
        draw_card_box(hdc, &box_rect, check_bg, check_border);

        if checked {
            SetTextColor(hdc, rgb(255, 255, 255));
            let old_f = SelectObject(hdc, font_bold.into());
            draw_text(
                hdc,
                "✓",
                box_rect.left,
                box_rect.top - 1,
                box_rect.right,
                box_rect.bottom,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            SelectObject(hdc, old_f);
        }

        // Title text
        let old_f = SelectObject(hdc, font_bold.into());
        SetTextColor(hdc, COLOR_TEXT_PRIMARY);
        draw_text(
            hdc,
            label,
            rect.left + 42,
            rect.top + 6,
            rect.right - 14,
            rect.top + 26,
            DT_LEFT | DT_SINGLELINE,
        );

        // Subtitle text
        if !description.is_empty() {
            SelectObject(hdc, font_small.into());
            SetTextColor(hdc, COLOR_TEXT_SECONDARY);
            draw_text(
                hdc,
                description,
                rect.left + 42,
                rect.top + 26,
                rect.right - 14,
                rect.bottom - 4,
                DT_LEFT | DT_SINGLELINE,
            );
        }

        SelectObject(hdc, old_f);
    }
}

// ---------------- Main Wizard Paint Handler ----------------

unsafe fn paint_wizard(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };

    let mut rect = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut rect) };
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    unsafe {
        // Double buffering
        let mem_dc = CreateCompatibleDC(Some(hdc));
        let mem_bm = CreateCompatibleBitmap(hdc, width, height);
        let old_bm = SelectObject(mem_dc, mem_bm.into());

        // Window background
        draw_rect_solid(mem_dc, &rect, COLOR_BG);

        // Subtle 1px window edge border
        let border_brush = CreateSolidBrush(COLOR_BORDER_SUBTLE);
        let b_top = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: 1,
        };
        let b_bot = RECT {
            left: 0,
            top: height - 1,
            right: width,
            bottom: height,
        };
        let b_lft = RECT {
            left: 0,
            top: 0,
            right: 1,
            bottom: height,
        };
        let b_rgt = RECT {
            left: width - 1,
            top: 0,
            right: width,
            bottom: height,
        };
        FillRect(mem_dc, &b_top, border_brush);
        FillRect(mem_dc, &b_bot, border_brush);
        FillRect(mem_dc, &b_lft, border_brush);
        FillRect(mem_dc, &b_rgt, border_brush);
        let _ = DeleteObject(border_brush.into());

        let state_opt = STATE.lock().unwrap().clone();
        if let Some(state) = state_opt {
            render_page_content(mem_dc, &state, width, height);
        }

        let _ = BitBlt(hdc, 0, 0, width, height, Some(mem_dc), 0, 0, SRCCOPY);
        SelectObject(mem_dc, old_bm);
        let _ = DeleteObject(mem_bm.into());
        let _ = DeleteDC(mem_dc);
        let _ = EndPaint(hwnd, &ps);
    }
}

unsafe fn render_page_content(hdc: HDC, state: &WizardState, w: i32, h: i32) {
    unsafe {
        SetBkMode(hdc, TRANSPARENT);

        let font_brand = create_font(20, 700);
        let font_title = create_font(16, 600);
        let font_body = create_font(13, 400);
        let font_bold = create_font(13, 600);
        let font_small = create_font(11, 400);
        let font_small_bold = create_font(11, 600);
        let font_btn = create_font(13, 600);

        // 1. Clean Top Header Bar (0..64px)
        let old_f = SelectObject(hdc, font_brand.into());
        SetTextColor(hdc, COLOR_TEXT_PRIMARY);

        // App Brand Title
        draw_text(hdc, "⚡ LiteCast", 26, 14, 180, 40, DT_LEFT | DT_SINGLELINE);

        // Version Tag Badge next to title
        let ver_badge = RECT {
            left: 148,
            top: 18,
            right: 218,
            bottom: 38,
        };
        draw_card_box(hdc, &ver_badge, COLOR_CARD, COLOR_BORDER_SUBTLE);
        SelectObject(hdc, font_small_bold.into());
        SetTextColor(hdc, COLOR_TEXT_SECONDARY);
        draw_text(
            hdc,
            &format!("v{}", APP_VERSION),
            ver_badge.left,
            ver_badge.top,
            ver_badge.right,
            ver_badge.bottom,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        // Subtitle
        SelectObject(hdc, font_small.into());
        SetTextColor(hdc, COLOR_TEXT_DIM);
        draw_text(
            hdc,
            "Windows Hızlı Başlatıcı & Sistem Yöneticisi",
            26,
            42,
            w - 100,
            60,
            DT_LEFT | DT_SINGLELINE,
        );

        // Top-Right Window Controls (Minimize & Close)
        // Minimize Button (998)
        let min_hover = state.hovered_btn == Some(998);
        let min_rect = RECT {
            left: w - 74,
            top: 14,
            right: w - 42,
            bottom: 42,
        };
        let min_bg = if min_hover {
            rgb(42, 44, 58)
        } else {
            rgb(26, 27, 36)
        };
        let min_border = if min_hover {
            rgb(70, 74, 98)
        } else {
            COLOR_BORDER_SUBTLE
        };
        draw_card_box(hdc, &min_rect, min_bg, min_border);
        SelectObject(hdc, font_bold.into());
        SetTextColor(hdc, COLOR_TEXT_SECONDARY);
        draw_text(
            hdc,
            "—",
            min_rect.left,
            min_rect.top,
            min_rect.right,
            min_rect.bottom,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        // Close Button (999)
        let close_hover = state.hovered_btn == Some(999);
        let close_rect = RECT {
            left: w - 38,
            top: 14,
            right: w - 10,
            bottom: 42,
        };
        let (close_bg, close_border, close_text) = if close_hover {
            (rgb(185, 28, 28), rgb(239, 68, 68), rgb(255, 255, 255))
        } else {
            (rgb(26, 27, 36), COLOR_BORDER_SUBTLE, COLOR_TEXT_SECONDARY)
        };
        draw_card_box(hdc, &close_rect, close_bg, close_border);
        SetTextColor(hdc, close_text);
        draw_text(
            hdc,
            "✕",
            close_rect.left,
            close_rect.top,
            close_rect.right,
            close_rect.bottom,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        // Header Divider Line
        let div_rect = RECT {
            left: 26,
            top: 64,
            right: w - 26,
            bottom: 65,
        };
        draw_rect_solid(hdc, &div_rect, COLOR_BORDER_SUBTLE);

        // 2. Subpage Content Rendering
        match &state.page {
            WizardPage::Welcome => {
                render_welcome_page(
                    hdc,
                    state,
                    w,
                    h,
                    font_title,
                    font_bold,
                    font_body,
                    font_small,
                    font_small_bold,
                    font_btn,
                );
            }
            WizardPage::Options => {
                render_options_page(
                    hdc,
                    state,
                    w,
                    h,
                    font_title,
                    font_bold,
                    font_body,
                    font_small,
                    font_small_bold,
                    font_btn,
                );
            }
            WizardPage::Processing {
                action_name,
                progress,
                logs,
            } => {
                render_processing_page(
                    hdc,
                    action_name,
                    *progress,
                    logs,
                    w,
                    h,
                    font_title,
                    font_body,
                    font_small,
                );
            }
            WizardPage::Completed { summary, is_repair } => {
                render_completed_page(
                    hdc, summary, *is_repair, state, w, h, font_title, font_bold, font_body,
                    font_small, font_btn,
                );
            }
            WizardPage::Uninstalled => {
                render_uninstalled_page(hdc, w, h, font_title, font_body, font_btn);
            }
        }

        SelectObject(hdc, old_f);
        let _ = DeleteObject(font_brand.into());
        let _ = DeleteObject(font_title.into());
        let _ = DeleteObject(font_body.into());
        let _ = DeleteObject(font_bold.into());
        let _ = DeleteObject(font_small.into());
        let _ = DeleteObject(font_small_bold.into());
        let _ = DeleteObject(font_btn.into());
    }
}

// ---------------- Subpage Renderers ----------------

#[allow(clippy::too_many_arguments)]
unsafe fn render_welcome_page(
    hdc: HDC,
    state: &WizardState,
    w: i32,
    _h: i32,
    _font_title: HFONT,
    font_bold: HFONT,
    font_body: HFONT,
    font_small: HFONT,
    font_small_bold: HFONT,
    font_btn: HFONT,
) {
    unsafe {
        if state.installation_info.is_already_installed {
            // Main Clean Unified Status Card (y: 80..260, Height 180px)
            let card_rect = RECT {
                left: 26,
                top: 80,
                right: w - 26,
                bottom: 260,
            };
            draw_card_box(hdc, &card_rect, COLOR_CARD, COLOR_BORDER);

            let is_valid = state.installation_info.is_integrity_valid;

            // Row 1: Clean Status Headline (y: 96..120)
            let status_dot = "●";
            let (dot_color, status_text) = if is_valid {
                (
                    COLOR_SUCCESS_TEXT,
                    "LiteCast sistemde kurulu ve dosya bütünlüğü tam.",
                )
            } else {
                (
                    COLOR_WARNING_TEXT,
                    "LiteCast kurulu, ancak dosya farkı tespit edildi (Onarım önerilir).",
                )
            };

            SelectObject(hdc, font_bold.into());
            SetTextColor(hdc, dot_color);
            draw_text(hdc, status_dot, 42, 96, 56, 120, DT_LEFT | DT_SINGLELINE);

            SetTextColor(
                hdc,
                if is_valid {
                    COLOR_TEXT_PRIMARY
                } else {
                    COLOR_WARNING_TEXT
                },
            );
            draw_text(
                hdc,
                status_text,
                58,
                96,
                w - 42,
                120,
                DT_LEFT | DT_SINGLELINE,
            );

            // Row 2: Target Location Label (y: 130)
            SelectObject(hdc, font_small_bold.into());
            SetTextColor(hdc, COLOR_TEXT_DIM);
            draw_text(
                hdc,
                "KURULUM KONUMU",
                42,
                130,
                w - 42,
                146,
                DT_LEFT | DT_SINGLELINE,
            );

            // Path Input Box (y: 150..184)
            let browse_btn_w = 96;
            let path_box = RECT {
                left: 42,
                top: 150,
                right: w - 42 - browse_btn_w - 8,
                bottom: 184,
            };
            draw_card_box(hdc, &path_box, COLOR_CARD_INNER, COLOR_BORDER_SUBTLE);
            SelectObject(hdc, font_body.into());
            SetTextColor(hdc, COLOR_TEXT_PRIMARY);
            draw_text(
                hdc,
                &state.install_dir.display().to_string(),
                52,
                150,
                path_box.right - 8,
                184,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_PATH_ELLIPSIS,
            );

            // Browse / Change Button (ID 100, y: 150..184)
            let browse_rect = RECT {
                left: w - 42 - browse_btn_w,
                top: 150,
                right: w - 42,
                bottom: 184,
            };
            draw_button(
                hdc,
                browse_rect,
                "Gözat...",
                state.hovered_btn == Some(100),
                false,
                false,
                font_btn,
            );

            // Row 3: Minimalist Metadata Summary (y: 202..244)
            SelectObject(hdc, font_small.into());
            let mid_x = (w - 84) / 2 + 42;

            // Col 1: Hash Bütünlüğü
            SetTextColor(hdc, COLOR_TEXT_DIM);
            draw_text(
                hdc,
                "• Bütünlük: ",
                42,
                204,
                115,
                224,
                DT_LEFT | DT_SINGLELINE,
            );
            if is_valid {
                SetTextColor(hdc, COLOR_SUCCESS_TEXT);
                draw_text(
                    hdc,
                    "SHA-256 Doğrulandı",
                    115,
                    204,
                    mid_x - 10,
                    224,
                    DT_LEFT | DT_SINGLELINE,
                );
            } else {
                SetTextColor(hdc, COLOR_WARNING_TEXT);
                draw_text(
                    hdc,
                    "Farklılık Var",
                    115,
                    204,
                    mid_x - 10,
                    224,
                    DT_LEFT | DT_SINGLELINE,
                );
            }

            // Col 2: Entegrasyon
            SetTextColor(hdc, COLOR_TEXT_DIM);
            draw_text(
                hdc,
                "• Başlangıç: ",
                mid_x,
                204,
                mid_x + 75,
                224,
                DT_LEFT | DT_SINGLELINE,
            );
            SetTextColor(hdc, COLOR_TEXT_SECONDARY);
            draw_text(
                hdc,
                if state.installation_info.autostart_enabled {
                    "Açılışta Aktif"
                } else {
                    "Pasif"
                },
                mid_x + 75,
                204,
                w - 42,
                224,
                DT_LEFT | DT_SINGLELINE,
            );

            // Col 1 Sub: Kısayollar
            SetTextColor(hdc, COLOR_TEXT_DIM);
            draw_text(
                hdc,
                "• Kısayollar: ",
                42,
                226,
                115,
                246,
                DT_LEFT | DT_SINGLELINE,
            );
            SetTextColor(hdc, COLOR_TEXT_SECONDARY);
            draw_text(
                hdc,
                &format!(
                    "Masaüstü: {}  |  Başlat Menüsü: {}",
                    if state.installation_info.desktop_shortcut_exists {
                        "Var"
                    } else {
                        "Yok"
                    },
                    if state.installation_info.startmenu_shortcut_exists {
                        "Var"
                    } else {
                        "Yok"
                    }
                ),
                115,
                226,
                w - 42,
                246,
                DT_LEFT | DT_SINGLELINE,
            );

            // Action Buttons
            // Row 1: Two Half-Width Symmetrical Action Buttons (y: 280..328)
            let btn_gap = 12;
            let btn_w = (w - 52 - btn_gap) / 2;

            // Button 0: Verify & Repair (Left)
            draw_button(
                hdc,
                RECT {
                    left: 26,
                    top: 280,
                    right: 26 + btn_w,
                    bottom: 328,
                },
                "🔍  Dosyaları Doğrula & Onar",
                state.hovered_btn == Some(0),
                true,
                false,
                font_btn,
            );

            // Button 1: Reinstall / Settings (Right)
            draw_button(
                hdc,
                RECT {
                    left: 26 + btn_w + btn_gap,
                    top: 280,
                    right: w - 26,
                    bottom: 328,
                },
                "⚙️  Yeniden Kur / Ayarlar",
                state.hovered_btn == Some(1),
                false,
                false,
                font_btn,
            );

            // Row 2: Subtle Danger Uninstall Button (y: 342..382)
            draw_button(
                hdc,
                RECT {
                    left: 26,
                    top: 342,
                    right: w - 26,
                    bottom: 382,
                },
                "🗑️  LiteCast'i Bilgisayardan Kaldır (Uninstall)",
                state.hovered_btn == Some(2),
                false,
                true,
                font_btn,
            );

            // Footer Hint (y: 404..424)
            SelectObject(hdc, font_small.into());
            SetTextColor(hdc, COLOR_TEXT_DIM);
            draw_text(
                hdc,
                "İpucu: Onarım işlemi mevcut ayarlarınızı koruyarak program dosyalarını yeniler.",
                26,
                404,
                w - 26,
                424,
                DT_CENTER | DT_SINGLELINE,
            );
        } else {
            // Fresh Installation Card (y: 80..270)
            let card_rect = RECT {
                left: 26,
                top: 80,
                right: w - 26,
                bottom: 270,
            };
            draw_card_box(hdc, &card_rect, COLOR_CARD, COLOR_BORDER);

            SelectObject(hdc, font_bold.into());
            SetTextColor(hdc, COLOR_TEXT_PRIMARY);
            draw_text(
                hdc,
                "LiteCast Kurulumuna Hoş Geldiniz",
                44,
                100,
                w - 44,
                126,
                DT_LEFT | DT_SINGLELINE,
            );

            SelectObject(hdc, font_body.into());
            SetTextColor(hdc, COLOR_TEXT_SECONDARY);
            draw_text(
                hdc,
                "LiteCast, Windows için geliştirilmiş ultra hafif, modern ve ışık hızında arama başlatıcısıdır.",
                44,
                130,
                w - 44,
                156,
                DT_LEFT | DT_SINGLELINE,
            );

            // Target Location Box
            SelectObject(hdc, font_small_bold.into());
            SetTextColor(hdc, COLOR_TEXT_DIM);
            draw_text(
                hdc,
                "HEDEF KURULUM DİZİNİ",
                44,
                168,
                w - 44,
                184,
                DT_LEFT | DT_SINGLELINE,
            );

            let browse_btn_w = 96;
            let path_box = RECT {
                left: 44,
                top: 188,
                right: w - 44 - browse_btn_w - 8,
                bottom: 222,
            };
            draw_card_box(hdc, &path_box, COLOR_CARD_INNER, COLOR_BORDER_SUBTLE);
            SelectObject(hdc, font_body.into());
            SetTextColor(hdc, COLOR_TEXT_PRIMARY);
            draw_text(
                hdc,
                &state.install_dir.display().to_string(),
                54,
                188,
                path_box.right - 8,
                222,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_PATH_ELLIPSIS,
            );

            let browse_rect = RECT {
                left: w - 44 - browse_btn_w,
                top: 188,
                right: w - 44,
                bottom: 222,
            };
            draw_button(
                hdc,
                browse_rect,
                "Gözat...",
                state.hovered_btn == Some(100),
                false,
                false,
                font_btn,
            );

            // Proceed Button (y: 298..348)
            draw_button(
                hdc,
                RECT {
                    left: 26,
                    top: 298,
                    right: w - 26,
                    bottom: 348,
                },
                "🚀  Kurulum Seçeneklerine İlerle",
                state.hovered_btn == Some(0),
                true,
                false,
                font_btn,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn render_options_page(
    hdc: HDC,
    state: &WizardState,
    w: i32,
    _h: i32,
    _font_title: HFONT,
    font_bold: HFONT,
    font_body: HFONT,
    font_small: HFONT,
    font_small_bold: HFONT,
    font_btn: HFONT,
) {
    unsafe {
        // Kurulum Dizini preview card with Browse button (y: 78..136)
        let dir_card = RECT {
            left: 26,
            top: 78,
            right: w - 26,
            bottom: 136,
        };
        draw_card_box(hdc, &dir_card, COLOR_CARD, COLOR_BORDER);

        SelectObject(hdc, font_small_bold.into());
        SetTextColor(hdc, COLOR_TEXT_DIM);
        draw_text(
            hdc,
            "HEDEF KURULUM KLASÖRÜ",
            40,
            86,
            w - 40,
            102,
            DT_LEFT | DT_SINGLELINE,
        );

        let browse_btn_w = 90;
        let path_box = RECT {
            left: 40,
            top: 104,
            right: w - 40 - browse_btn_w - 8,
            bottom: 128,
        };
        SelectObject(hdc, font_body.into());
        SetTextColor(hdc, COLOR_TEXT_PRIMARY);
        draw_text(
            hdc,
            &state.install_dir.display().to_string(),
            path_box.left,
            path_box.top,
            path_box.right,
            path_box.bottom,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_PATH_ELLIPSIS,
        );

        let browse_rect = RECT {
            left: w - 40 - browse_btn_w,
            top: 96,
            right: w - 40,
            bottom: 126,
        };
        draw_button(
            hdc,
            browse_rect,
            "Gözat...",
            state.hovered_btn == Some(100),
            false,
            false,
            font_small_bold,
        );

        // 3 Checkbox Rows (y: 148, 206, 264)
        draw_checkbox_row(
            hdc,
            RECT {
                left: 26,
                top: 148,
                right: w - 26,
                bottom: 196,
            },
            state.opt_autostart,
            "Windows ile birlikte otomatik başlat",
            "Windows açıldığında LiteCast arka planda hazır ve hızlı erişilebilir bekler",
            state.hovered_btn == Some(101),
            font_bold,
            font_small,
        );

        draw_checkbox_row(
            hdc,
            RECT {
                left: 26,
                top: 206,
                right: w - 26,
                bottom: 254,
            },
            state.opt_desktop_shortcut,
            "Masaüstü kısayolu oluştur",
            "Masaüstünüze doğrudan hızlı başlatma simgesi ekler",
            state.hovered_btn == Some(102),
            font_bold,
            font_small,
        );

        draw_checkbox_row(
            hdc,
            RECT {
                left: 26,
                top: 264,
                right: w - 26,
                bottom: 312,
            },
            state.opt_startmenu_shortcut,
            "Başlat Menüsü kısayolu oluştur",
            "Windows Başlat menüsü ve arama indeksinden erişilebilir kılar",
            state.hovered_btn == Some(103),
            font_bold,
            font_small,
        );

        // Bottom Navigation Buttons (y: 340..390)
        // Left: Back (Width 130px)
        draw_button(
            hdc,
            RECT {
                left: 26,
                top: 340,
                right: 156,
                bottom: 390,
            },
            "← Geri",
            state.hovered_btn == Some(10),
            false,
            false,
            font_btn,
        );

        // Right: Start Install
        draw_button(
            hdc,
            RECT {
                left: 168,
                top: 340,
                right: w - 26,
                bottom: 390,
            },
            "🚀  Kurulumu Başlat",
            state.hovered_btn == Some(11),
            true,
            false,
            font_btn,
        );
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn render_processing_page(
    hdc: HDC,
    action_name: &str,
    progress: usize,
    logs: &[String],
    w: i32,
    _h: i32,
    font_title: HFONT,
    font_body: HFONT,
    font_small: HFONT,
) {
    unsafe {
        SelectObject(hdc, font_title.into());
        SetTextColor(hdc, COLOR_TEXT_PRIMARY);
        draw_text(
            hdc,
            &format!("İşlem Gerçekleştiriliyor: {}...", action_name),
            26,
            82,
            w - 26,
            108,
            DT_LEFT | DT_SINGLELINE,
        );

        // Progress Bar Track
        let bar_rect = RECT {
            left: 26,
            top: 120,
            right: w - 26,
            bottom: 142,
        };
        draw_card_box(hdc, &bar_rect, COLOR_CARD_INNER, COLOR_BORDER);

        // Progress Bar Fill
        let fill_w =
            (((w - 54) as f32) * (progress as f32 / 100.0)).clamp(0.0, (w - 54) as f32) as i32;
        if fill_w > 0 {
            let fill_rect = RECT {
                left: 27,
                top: 121,
                right: 27 + fill_w,
                bottom: 141,
            };
            draw_rect_solid(hdc, &fill_rect, COLOR_ACCENT);
        }

        // Percentage Label in Bar
        SelectObject(hdc, font_small.into());
        SetTextColor(hdc, rgb(255, 255, 255));
        draw_text(
            hdc,
            &format!("{}%", progress),
            bar_rect.left,
            bar_rect.top,
            bar_rect.right,
            bar_rect.bottom,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        // Terminal Log Box (y: 160..390)
        let log_box = RECT {
            left: 26,
            top: 160,
            right: w - 26,
            bottom: 390,
        };
        draw_card_box(hdc, &log_box, rgb(13, 14, 18), COLOR_BORDER);

        SelectObject(hdc, font_body.into());
        SetTextColor(hdc, COLOR_TEXT_SECONDARY);

        let mut start_y = 174;
        for line in logs.iter().rev().take(7).rev() {
            draw_text(
                hdc,
                line,
                40,
                start_y,
                w - 40,
                start_y + 22,
                DT_LEFT | DT_SINGLELINE,
            );
            start_y += 26;
        }
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn render_completed_page(
    hdc: HDC,
    summary: &str,
    is_repair: bool,
    state: &WizardState,
    w: i32,
    _h: i32,
    font_title: HFONT,
    font_bold: HFONT,
    font_body: HFONT,
    font_small: HFONT,
    font_btn: HFONT,
) {
    unsafe {
        SelectObject(hdc, font_title.into());
        SetTextColor(hdc, COLOR_SUCCESS_TEXT);

        let title_msg = if is_repair {
            "✓  Doğrulama ve Onarım Başarıyla Tamamlandı!"
        } else {
            "✓  LiteCast Kurulumu Başarıyla Tamamlandı!"
        };
        draw_text(hdc, title_msg, 26, 82, w - 26, 110, DT_LEFT | DT_SINGLELINE);

        // Summary Card (y: 120..240)
        let card_rect = RECT {
            left: 26,
            top: 120,
            right: w - 26,
            bottom: 240,
        };
        draw_card_box(hdc, &card_rect, COLOR_CARD, COLOR_BORDER);

        SelectObject(hdc, font_body.into());
        SetTextColor(hdc, COLOR_TEXT_PRIMARY);
        draw_text(hdc, summary, 44, 136, w - 44, 224, DT_LEFT | DT_WORDBREAK);

        // Checkbox: Launch After Finish (y: 260..308)
        draw_checkbox_row(
            hdc,
            RECT {
                left: 26,
                top: 260,
                right: w - 26,
                bottom: 308,
            },
            state.opt_launch_after,
            "LiteCast'i Şimdi Başlat",
            "Sihirbaz kapandığında LiteCast anında kullanıma açılır",
            state.hovered_btn == Some(120),
            font_bold,
            font_small,
        );

        // Finish Button (y: 330..380)
        draw_button(
            hdc,
            RECT {
                left: 26,
                top: 330,
                right: w - 26,
                bottom: 380,
            },
            "Tamamla ve Kapat (Finish)",
            state.hovered_btn == Some(20),
            true,
            false,
            font_btn,
        );
    }
}

unsafe fn render_uninstalled_page(
    hdc: HDC,
    w: i32,
    _h: i32,
    font_title: HFONT,
    font_body: HFONT,
    font_btn: HFONT,
) {
    unsafe {
        SelectObject(hdc, font_title.into());
        SetTextColor(hdc, COLOR_TEXT_PRIMARY);
        draw_text(
            hdc,
            "LiteCast Sistemden Kaldırıldı",
            26,
            90,
            w - 26,
            120,
            DT_LEFT | DT_SINGLELINE,
        );

        let card_rect = RECT {
            left: 26,
            top: 136,
            right: w - 26,
            bottom: 240,
        };
        draw_card_box(hdc, &card_rect, COLOR_CARD, COLOR_BORDER);

        SelectObject(hdc, font_body.into());
        SetTextColor(hdc, COLOR_TEXT_SECONDARY);
        draw_text(
            hdc,
            "Tüm program dosyaları, kısayollar ve Windows başlangıç kayıtları sisteminizden tamamen temizlendi.\n\nLiteCast'i tercih ettiğiniz için teşekkür ederiz!",
            44,
            152,
            w - 44,
            224,
            DT_LEFT | DT_WORDBREAK,
        );

        draw_button(
            hdc,
            RECT {
                left: 26,
                top: 280,
                right: w - 26,
                bottom: 330,
            },
            "Kapat",
            true,
            false,
            false,
            font_btn,
        );
    }
}

// ---------------- Window Procedure & Events ----------------

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xffff) as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i32;
            handle_mouse_move(hwnd, x, y);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xffff) as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i32;
            handle_mouse_click(hwnd, x, y);
            LRESULT(0)
        }
        WM_APP_PROGRESS_UPDATE => {
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_APP_ACTION_DONE => {
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wparam.0 as u32;
            if vk == 0x1B {
                // ESC to exit
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_PAINT => {
            unsafe {
                paint_wizard(hwnd);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn handle_mouse_move(hwnd: HWND, x: i32, y: i32) {
    let mut rect = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut rect) };
    let w = rect.right - rect.left;

    let mut need_repaint = false;
    let mut is_hand = false;

    {
        let mut guard = STATE.lock().unwrap();
        if let Some(state) = guard.as_mut() {
            let old_hover = state.hovered_btn;
            state.hovered_btn = None;

            // Top-right window controls
            if (14..=42).contains(&y) {
                if ((w - 74)..=(w - 42)).contains(&x) {
                    state.hovered_btn = Some(998); // Minimize
                    is_hand = true;
                } else if ((w - 38)..=(w - 10)).contains(&x) {
                    state.hovered_btn = Some(999); // Close
                    is_hand = true;
                }
            }

            if state.hovered_btn.is_none() {
                match &state.page {
                    WizardPage::Welcome => {
                        let browse_btn_w = 96;
                        if state.installation_info.is_already_installed {
                            let btn_gap = 12;
                            let btn_w = (w - 52 - btn_gap) / 2;

                            // Browse button
                            if (150..=184).contains(&y)
                                && ((w - 42 - browse_btn_w)..=(w - 42)).contains(&x)
                            {
                                state.hovered_btn = Some(100);
                                is_hand = true;
                            } else if (280..=328).contains(&y) {
                                if (26..=(26 + btn_w)).contains(&x) {
                                    state.hovered_btn = Some(0); // Verify & Repair
                                    is_hand = true;
                                } else if ((26 + btn_w + btn_gap)..=(w - 26)).contains(&x) {
                                    state.hovered_btn = Some(1); // Reinstall
                                    is_hand = true;
                                }
                            } else if (342..=382).contains(&y) && (26..=(w - 26)).contains(&x) {
                                state.hovered_btn = Some(2); // Uninstall
                                is_hand = true;
                            }
                        } else {
                            if (188..=222).contains(&y)
                                && ((w - 44 - browse_btn_w)..=(w - 44)).contains(&x)
                            {
                                state.hovered_btn = Some(100);
                                is_hand = true;
                            } else if (298..=348).contains(&y) && (26..=(w - 26)).contains(&x) {
                                state.hovered_btn = Some(0);
                                is_hand = true;
                            }
                        }
                    }
                    WizardPage::Options => {
                        let browse_btn_w = 90;
                        if (96..=126).contains(&y)
                            && ((w - 40 - browse_btn_w)..=(w - 40)).contains(&x)
                        {
                            state.hovered_btn = Some(100);
                            is_hand = true;
                        } else if (148..=196).contains(&y) && (26..=(w - 26)).contains(&x) {
                            state.hovered_btn = Some(101);
                            is_hand = true;
                        } else if (206..=254).contains(&y) && (26..=(w - 26)).contains(&x) {
                            state.hovered_btn = Some(102);
                            is_hand = true;
                        } else if (264..=312).contains(&y) && (26..=(w - 26)).contains(&x) {
                            state.hovered_btn = Some(103);
                            is_hand = true;
                        } else if (340..=390).contains(&y) {
                            if (26..=156).contains(&x) {
                                state.hovered_btn = Some(10); // Back
                                is_hand = true;
                            } else if (168..=(w - 26)).contains(&x) {
                                state.hovered_btn = Some(11); // Start Install
                                is_hand = true;
                            }
                        }
                    }
                    WizardPage::Completed { .. } => {
                        if (260..=308).contains(&y) && (26..=(w - 26)).contains(&x) {
                            state.hovered_btn = Some(120);
                            is_hand = true;
                        } else if (330..=380).contains(&y) && (26..=(w - 26)).contains(&x) {
                            state.hovered_btn = Some(20);
                            is_hand = true;
                        }
                    }
                    WizardPage::Uninstalled => {
                        if (280..=330).contains(&y) && (26..=(w - 26)).contains(&x) {
                            state.hovered_btn = Some(30);
                            is_hand = true;
                        }
                    }
                    _ => {}
                }
            }

            if old_hover != state.hovered_btn {
                need_repaint = true;
            }
        }
    }

    unsafe {
        let cursor_id = if is_hand { IDC_HAND } else { IDC_ARROW };
        if let Ok(cur) = LoadCursorW(None, cursor_id) {
            let _ = SetCursor(Some(cur));
        }
        if need_repaint {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }
}

fn handle_mouse_click(hwnd: HWND, x: i32, y: i32) {
    let mut rect = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut rect) };
    let w = rect.right - rect.left;

    // Top-right window controls
    if (14..=42).contains(&y) {
        if ((w - 74)..=(w - 42)).contains(&x) {
            // Minimize
            unsafe {
                let _ = ShowWindow(hwnd, SW_MINIMIZE);
            }
            return;
        } else if ((w - 38)..=(w - 10)).contains(&x) {
            // Close
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            return;
        }
    }

    // Window Dragging on top bar
    if y <= 64 && x < (w - 80) {
        unsafe {
            let _ = ReleaseCapture();
            let _ = SendMessageW(
                hwnd,
                WM_NCLBUTTONDOWN,
                Some(WPARAM(HTCAPTION as usize)),
                Some(LPARAM(0)),
            );
        }
        return;
    }

    let mut next_page: Option<WizardPage> = None;
    let mut should_close = false;
    let mut trigger_action: Option<&'static str> = None;
    let mut open_browse = false;

    {
        let mut guard = STATE.lock().unwrap();
        if let Some(state) = guard.as_mut() {
            match &state.page {
                WizardPage::Welcome => {
                    let browse_btn_w = 96;
                    if state.installation_info.is_already_installed {
                        let btn_gap = 12;
                        let btn_w = (w - 52 - btn_gap) / 2;

                        if (150..=184).contains(&y)
                            && ((w - 42 - browse_btn_w)..=(w - 42)).contains(&x)
                        {
                            open_browse = true;
                        } else if (280..=328).contains(&y) {
                            if (26..=(26 + btn_w)).contains(&x) {
                                // Verify & Repair
                                trigger_action = Some("repair");
                            } else if ((26 + btn_w + btn_gap)..=(w - 26)).contains(&x) {
                                // Reinstall / Options
                                next_page = Some(WizardPage::Options);
                            }
                        } else if (342..=382).contains(&y) && (26..=(w - 26)).contains(&x) {
                            // Uninstall
                            trigger_action = Some("uninstall");
                        }
                    } else {
                        if (188..=222).contains(&y)
                            && ((w - 44 - browse_btn_w)..=(w - 44)).contains(&x)
                        {
                            open_browse = true;
                        } else if (298..=348).contains(&y) && (26..=(w - 26)).contains(&x) {
                            next_page = Some(WizardPage::Options);
                        }
                    }
                }
                WizardPage::Options => {
                    let browse_btn_w = 90;
                    if (96..=126).contains(&y) && ((w - 40 - browse_btn_w)..=(w - 40)).contains(&x)
                    {
                        open_browse = true;
                    } else if (148..=196).contains(&y) && (26..=(w - 26)).contains(&x) {
                        state.opt_autostart = !state.opt_autostart;
                    } else if (206..=254).contains(&y) && (26..=(w - 26)).contains(&x) {
                        state.opt_desktop_shortcut = !state.opt_desktop_shortcut;
                    } else if (264..=312).contains(&y) && (26..=(w - 26)).contains(&x) {
                        state.opt_startmenu_shortcut = !state.opt_startmenu_shortcut;
                    } else if (340..=390).contains(&y) {
                        if (26..=156).contains(&x) {
                            next_page = Some(WizardPage::Welcome);
                        } else if (168..=(w - 26)).contains(&x) {
                            trigger_action = Some("install");
                        }
                    }
                }
                WizardPage::Completed { .. } => {
                    if (260..=308).contains(&y) && (26..=(w - 26)).contains(&x) {
                        state.opt_launch_after = !state.opt_launch_after;
                    } else if (330..=380).contains(&y) && (26..=(w - 26)).contains(&x) {
                        if state.opt_launch_after {
                            let bin = state.install_dir.join("LiteCast.exe");
                            start_installed_litecast(&bin);
                        }
                        should_close = true;
                    }
                }
                WizardPage::Uninstalled => {
                    if (280..=330).contains(&y) && (26..=(w - 26)).contains(&x) {
                        should_close = true;
                    }
                }
                _ => {}
            }

            if let Some(np) = next_page {
                state.page = np;
            }
        }
    }

    if open_browse {
        let cur_dir = {
            let guard = STATE.lock().unwrap();
            guard
                .as_ref()
                .map(|s| s.install_dir.clone())
                .unwrap_or_else(get_default_install_dir)
        };

        if let Some(new_dir) = pick_install_directory(hwnd, &cur_dir) {
            let new_info = detect_system_state(&new_dir);
            let mut guard = STATE.lock().unwrap();
            if let Some(state) = guard.as_mut() {
                state.install_dir = new_dir;
                state.installation_info = new_info;
            }
        }
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
        return;
    }

    if let Some(act) = trigger_action {
        match act {
            "install" => {
                run_async_install_or_repair(hwnd, false);
            }
            "repair" => {
                run_async_install_or_repair(hwnd, true);
            }
            "uninstall" => {
                run_async_uninstall(hwnd);
            }
            _ => {}
        }
        return;
    }

    if should_close {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    } else {
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }
}

// ---------------- Main Entry Point ----------------

fn main() {
    let default_dir = get_default_install_dir();
    let info = detect_system_state(&default_dir);

    {
        let mut guard = STATE.lock().unwrap();
        *guard = Some(WizardState {
            page: WizardPage::Welcome,
            install_dir: default_dir,
            opt_autostart: false, // LC-007: opt_autostart default false (opt-in)
            opt_desktop_shortcut: true,
            opt_startmenu_shortcut: true,
            opt_launch_after: true,
            installation_info: info,
            hovered_btn: None,
        });
    }

    let args: Vec<String> = std::env::args().collect();

    // CLI mode: Silent Install (LC-010)
    if args
        .iter()
        .any(|a| a == "--silent" || a == "/S" || a == "/silent" || a == "--quiet")
    {
        match perform_silent_install_sync() {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                eprintln!("Silent kurulum hatası: {}", e);
                std::process::exit(1);
            }
        }
    }

    // CLI mode: Uninstall (LC-011)
    if args.iter().any(|a| a == "--uninstall" || a == "-u") {
        match perform_uninstall_sync() {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                eprintln!("Kaldırma işlemi başarısız: {}", e);
                std::process::exit(1);
            }
        }
    }

    // CLI mode: Verify Only (Read-Only integrity check, LC-009)
    if args.iter().any(|a| a == "--verify" || a == "-v") {
        match perform_verify_only_sync() {
            Ok(true) => std::process::exit(0),
            Ok(false) => std::process::exit(1),
            Err(e) => {
                eprintln!("Doğrulama hatası: {}", e);
                std::process::exit(2);
            }
        }
    }

    // CLI mode: Repair (LC-009)
    if args.iter().any(|a| a == "--repair" || a == "-r") {
        match perform_install_or_repair_sync(true) {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                eprintln!("Onarım başarısız: {}", e);
                std::process::exit(1);
            }
        }
    }

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        let hinstance = GetModuleHandleW(PCWSTR::null()).unwrap_or_default();
        let class_name = to_wide("LiteCastInstallerClass");

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance.into(),
            hIcon: Default::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: Default::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: PCWSTR::from_raw(class_name.as_ptr()),
        };

        RegisterClassW(&wc);

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_w - WND_WIDTH) / 2;
        let y = (screen_h - WND_HEIGHT) / 2;

        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            PCWSTR::from_raw(class_name.as_ptr()),
            PCWSTR::from_raw(to_wide("LiteCast Kurulum Sihirbazı").as_ptr()),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            WND_WIDTH,
            WND_HEIGHT,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
        .unwrap_or_default();

        if !hwnd.0.is_null() {
            // Apply Windows 11 rounded corners & dark mode attributes
            let dark_mode: i32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(20), // DWMWA_USE_IMMERSIVE_DARK_MODE
                &dark_mode as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            );

            let corner_pref: i32 = 2; // DWMWCP_ROUND
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(33), // DWMWA_WINDOW_CORNER_PREFERENCE
                &corner_pref as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            );

            let _ = SetWindowPos(
                hwnd,
                None,
                x,
                y,
                WND_WIDTH,
                WND_HEIGHT,
                SWP_NOZORDER | SWP_SHOWWINDOW,
            );
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

// Synchronous CLI Helpers for CLI flags

/// LC-009: Read-only verification (No disk/registry modifications)
fn perform_verify_only_sync() -> Result<bool, String> {
    let install_dir = get_registered_install_dir().unwrap_or_else(get_default_install_dir);
    let state = detect_system_state(&install_dir);
    if !state.is_already_installed {
        eprintln!(
            "LiteCast belirtilen konumda kurulu değil: {}",
            install_dir.display()
        );
        return Ok(false);
    }
    if !state.is_integrity_valid {
        eprintln!(
            "Bütünlük hatası! Beklenen SHA-256: {}, Mevcut SHA-256: {}",
            state.expected_hash, state.installed_hash
        );
        return Ok(false);
    }
    println!(
        "LiteCast kurulumu geçerli ve sağlam: {}",
        install_dir.display()
    );
    Ok(true)
}

/// LC-010: Silent, headless installation
fn perform_silent_install_sync() -> Result<(), String> {
    let default_dir = get_default_install_dir();
    stop_running_litecast();
    let target = default_dir.join("LiteCast.exe");
    fs::create_dir_all(&default_dir)
        .map_err(|e| format!("Kurulum dizini oluşturulamadı: {}", e))?;
    fs::write(&target, EMBEDDED_BINARY).map_err(|e| format!("LiteCast.exe yazılamadı: {}", e))?;

    if let Ok(current_setup) = std::env::current_exe() {
        let uninstaller_dest = default_dir.join("LiteCast-Setup.exe");
        let _ = fs::copy(&current_setup, &uninstaller_dest);
    }

    register_uninstall_entry(&default_dir, &target)?;
    let _ = create_shortcut(&target, &get_desktop_shortcut_path(), "LiteCast");
    let _ = create_shortcut(&target, &get_start_menu_shortcut_path(), "LiteCast");
    Ok(())
}

fn perform_install_or_repair_sync(is_repair: bool) -> Result<String, String> {
    let default_dir = get_default_install_dir();
    stop_running_litecast();
    let target = default_dir.join("LiteCast.exe");
    fs::create_dir_all(&default_dir).map_err(|e| e.to_string())?;
    fs::write(&target, EMBEDDED_BINARY).map_err(|e| e.to_string())?;

    if let Ok(current_setup) = std::env::current_exe() {
        let uninstaller_dest = default_dir.join("LiteCast-Setup.exe");
        let _ = fs::copy(&current_setup, &uninstaller_dest);
    }

    let _ = set_registry_autostart(&target, false); // Default false for safe opt-in
    register_uninstall_entry(&default_dir, &target)?;
    let _ = create_shortcut(&target, &get_desktop_shortcut_path(), "LiteCast");
    let _ = create_shortcut(&target, &get_start_menu_shortcut_path(), "LiteCast");
    Ok(if is_repair {
        "Onarım tamamlandı"
    } else {
        "Kurulum tamamlandı"
    }
    .into())
}

/// LC-011: Uninstall from actual registered location
fn perform_uninstall_sync() -> Result<(), String> {
    stop_running_litecast();
    let install_dir = get_registered_install_dir().unwrap_or_else(get_default_install_dir);
    let target = install_dir.join("LiteCast.exe");
    let _ = set_registry_autostart(&target, false);
    unregister_uninstall_entry();
    let _ = fs::remove_file(get_desktop_shortcut_path());
    let _ = fs::remove_file(get_start_menu_shortcut_path());
    if install_dir.exists() {
        let _ = fs::remove_dir_all(&install_dir);
    }
    Ok(())
}
