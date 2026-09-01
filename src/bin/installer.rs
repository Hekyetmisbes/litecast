#![windows_subsystem = "windows"]

// LiteCast Kurulum, Doğrulama & Bakım Sihirbazı (Installer & Verifier Wizard)
// Windows 10/11 için yerel, hızlı, koyu temalı Win32 kurulum yöneticisi.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreateSolidBrush,
    DRAW_TEXT_FORMAT, DT_CENTER, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK,
    DeleteDC, DeleteObject, DrawTextW, EndPaint, FONT_CHARSET, FONT_CLIP_PRECISION,
    FONT_OUTPUT_PRECISION, FONT_QUALITY, FillRect, FrameRect, HDC, HFONT, InvalidateRect,
    PAINTSTRUCT, SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegCreateKeyW,
    RegDeleteKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetClientRect, GetMessageW, GetSystemMetrics, IDC_ARROW, IDC_HAND, LoadCursorW, MSG,
    PostQuitMessage, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW, SWP_NOZORDER,
    SWP_SHOWWINDOW, SetCursor, SetForegroundWindow, SetWindowPos, ShowWindow, TranslateMessage,
    WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_PAINT, WNDCLASSW,
    WS_EX_APPWINDOW, WS_POPUP, WS_VISIBLE,
};
use windows::core::PCWSTR;

// Gömülü LiteCast binary'si (Dağıtım için standalone payload)
const EMBEDDED_BINARY: &[u8] = include_bytes!("../../LiteCast.exe");

const APP_NAME: &str = "LiteCast";
const APP_VERSION: &str = "1.0.0";
const PUBLISHER: &str = "LiteCast Team";
const RUN_REG_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const UNINSTALL_REG_PATH: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\LiteCast";

const WND_WIDTH: i32 = 640;
const WND_HEIGHT: i32 = 500;

// Dark Palette
const COLOR_BG: COLORREF = COLORREF(0x00171312); // #121317 (BGR: 0x171312)
const COLOR_CARD: COLORREF = COLORREF(0x00241d1b); // #1b1d24
const COLOR_BORDER: COLORREF = COLORREF(0x00332826); // #262833
const COLOR_ACCENT: COLORREF = COLORREF(0x00eb6325); // #2563eb (Blue)
const COLOR_ACCENT_HOVER: COLORREF = COLORREF(0x00f6823b); // #3b82f6
const COLOR_SUCCESS: COLORREF = COLORREF(0x0081b910); // #10b981 (Green)
const COLOR_DANGER: COLORREF = COLORREF(0x004444ef); // #ef4444 (Red)
const COLOR_TEXT: COLORREF = COLORREF(0x00f8faf0); // #f0faf8
const COLOR_TEXT_MUTED: COLORREF = COLORREF(0x00b8a394); // #94a3b8

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

fn detect_system_state() -> InstallationState {
    let install_dir = get_default_install_dir();
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

// ---------------- Windows Registry Helpers ----------------

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
            let mut buf = [0u8; 1024];
            let mut buf_len = buf.len() as u32;
            let res = RegQueryValueExW(
                hkey,
                PCWSTR::from_raw(wide_name.as_ptr()),
                None,
                None,
                Some(buf.as_mut_ptr()),
                Some(&mut buf_len),
            );
            let _ = RegCloseKey(hkey);
            if res.is_ok() {
                let val = String::from_utf8_lossy(&buf[..buf_len as usize]);
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
            if enable {
                let cmd_str = format!("\"{}\"", binary_path.display());
                let wide_val = to_wide(&cmd_str);
                let _ = RegSetValueExW(
                    hkey,
                    PCWSTR::from_raw(wide_name.as_ptr()),
                    None,
                    REG_SZ,
                    Some(std::slice::from_raw_parts(
                        wide_val.as_ptr() as *const u8,
                        wide_val.len() * 2,
                    )),
                );
            } else {
                let _ = RegDeleteValueW(hkey, PCWSTR::from_raw(wide_name.as_ptr()));
            }
            let _ = RegCloseKey(hkey);
            Ok(())
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
            let set_str = |key_name: &str, val_str: &str| {
                let w_key = to_wide(key_name);
                let w_val = to_wide(val_str);
                let _ = RegSetValueExW(
                    hkey,
                    PCWSTR::from_raw(w_key.as_ptr()),
                    None,
                    REG_SZ,
                    Some(std::slice::from_raw_parts(
                        w_val.as_ptr() as *const u8,
                        w_val.len() * 2,
                    )),
                );
            };

            set_str("DisplayName", APP_NAME);
            set_str("DisplayVersion", APP_VERSION);
            set_str("Publisher", PUBLISHER);
            set_str("InstallLocation", &install_dir.to_string_lossy());
            set_str("DisplayIcon", &format!("\"{}\",0", binary_path.display()));

            // Uninstall command via powershell uninstaller or installer binary
            let uninstaller_exe = install_dir.join("LiteCast-Setup.exe");
            if uninstaller_exe.exists() {
                set_str(
                    "UninstallString",
                    &format!("\"{}\" --uninstall", uninstaller_exe.display()),
                );
            } else {
                let ps_uninstall = format!(
                    "powershell.exe -NoProfile -Command \"Stop-Process -Name LiteCast -Force -ErrorAction SilentlyContinue; Remove-Item -Path '{}' -Recurse -Force\"",
                    install_dir.display()
                );
                set_str("UninstallString", &ps_uninstall);
            }

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

// ---------------- Shortcut Creation ----------------

fn create_shortcut(
    target_exe: &Path,
    shortcut_path: &Path,
    description: &str,
) -> Result<(), String> {
    if let Some(parent) = shortcut_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let ps_script = format!(
        "$WshShell = New-Object -ComObject WScript.Shell; \
         $Shortcut = $WshShell.CreateShortcut('{}'); \
         $Shortcut.TargetPath = '{}'; \
         $Shortcut.WorkingDirectory = '{}'; \
         $Shortcut.Description = '{}'; \
         $Shortcut.IconLocation = '{},0'; \
         $Shortcut.Save()",
        shortcut_path.to_string_lossy().replace('\'', "''"),
        target_exe.to_string_lossy().replace('\'', "''"),
        target_exe
            .parent()
            .unwrap_or(Path::new(""))
            .to_string_lossy()
            .replace('\'', "''"),
        description,
        target_exe.to_string_lossy().replace('\'', "''")
    );

    let status = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .status();

    match status {
        Ok(s) if s.success() => Ok(()),
        _ => Err(format!(
            "Kısayol oluşturulamadı: {}",
            shortcut_path.display()
        )),
    }
}

// ---------------- Process Management ----------------

fn stop_running_litecast() {
    let _ = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            "Stop-Process -Name LiteCast,litecast -Force -ErrorAction SilentlyContinue",
        ])
        .status();
    std::thread::sleep(std::time::Duration::from_millis(300));
}

fn start_installed_litecast(binary_path: &Path) {
    let _ = Command::new(binary_path).spawn();
}

// ---------------- Installation / Verification Actions ----------------

fn perform_install_or_repair(is_repair: bool) -> Result<String, String> {
    let state_snapshot = {
        let guard = STATE.lock().unwrap();
        guard.as_ref().cloned().ok_or("Durum alınamadı")?
    };

    let install_dir = state_snapshot.install_dir;
    let target_binary = install_dir.join("LiteCast.exe");
    let setup_binary_in_dir = install_dir.join("LiteCast-Setup.exe");

    // 1. Stop any active processes
    stop_running_litecast();

    // 2. Ensure install directory exists
    fs::create_dir_all(&install_dir).map_err(|e| format!("Hedef klasör oluşturulamadı: {}", e))?;

    // 3. Write LiteCast.exe binary payload
    fs::write(&target_binary, EMBEDDED_BINARY).map_err(|e| format!("Dosya yazılamadı: {}", e))?;

    // 4. Also copy current setup exe to install folder for seamless uninstaller / repair tool
    if let Ok(current_exe) = std::env::current_exe() {
        let _ = fs::copy(current_exe, &setup_binary_in_dir);
    }

    // 5. SHA-256 Verification check
    let written_bytes =
        fs::read(&target_binary).map_err(|e| format!("Doğrulama okuma hatası: {}", e))?;
    let computed_hash = compute_sha256(&written_bytes);
    let expected_hash = compute_sha256(EMBEDDED_BINARY);

    if computed_hash != expected_hash {
        return Err("Bütünlük doğrulaması başarısız: SHA-256 hash eşleşmedi!".into());
    }

    // 6. Registry Settings
    let _ = set_registry_autostart(&target_binary, state_snapshot.opt_autostart);
    let _ = register_uninstall_entry(&install_dir, &target_binary);

    // 7. Shortcuts
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

    let action_str = if is_repair {
        "Onarım ve Doğrulama"
    } else {
        "Kurulum"
    };
    Ok(format!(
        "{} başarıyla tamamlandı!\n• Hedef: {}\n• Bütünlük: SHA-256 doğrulandı ({:.2} MB)",
        action_str,
        target_binary.display(),
        (written_bytes.len() as f64) / (1024.0 * 1024.0)
    ))
}

fn perform_uninstall() -> Result<(), String> {
    stop_running_litecast();

    let install_dir = get_default_install_dir();
    let target_binary = install_dir.join("LiteCast.exe");

    // Remove autostart
    let _ = set_registry_autostart(&target_binary, false);
    unregister_uninstall_entry();

    // Remove shortcuts
    let desktop_lnk = get_desktop_shortcut_path();
    if desktop_lnk.exists() {
        let _ = fs::remove_file(desktop_lnk);
    }

    let start_lnk = get_start_menu_shortcut_path();
    if start_lnk.exists() {
        let _ = fs::remove_file(start_lnk);
    }

    // Remove program directory
    if install_dir.exists() {
        let _ = fs::remove_dir_all(&install_dir);
    }

    Ok(())
}

// ---------------- Modern Win32 GUI Rendering ----------------

unsafe fn paint_wizard(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };

    let mut rect = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut rect) };
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    // Double buffer
    unsafe {
        let mem_dc = CreateCompatibleDC(Some(hdc));
        let mem_bm = CreateCompatibleBitmap(hdc, width, height);
        let old_bm = SelectObject(mem_dc, mem_bm.into());

        // Background
        let bg_brush = CreateSolidBrush(COLOR_BG);
        FillRect(mem_dc, &rect, bg_brush);
        let _ = DeleteObject(bg_brush.into());

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

        // Header Area
        let header_font = create_font(26, 700);
        let body_font = create_font(16, 400);
        let bold_font = create_font(16, 600);
        let title_font = create_font(20, 600);

        let old_font = SelectObject(hdc, header_font.into());

        // Title & Icon
        SetTextColor(hdc, COLOR_TEXT);
        draw_text(
            hdc,
            "⚡ LiteCast Kurulum & Doğrulama Sihirbazı",
            32,
            24,
            w - 32,
            60,
            DT_LEFT | DT_SINGLELINE,
        );

        SelectObject(hdc, body_font.into());
        SetTextColor(hdc, COLOR_TEXT_MUTED);
        draw_text(
            hdc,
            &format!(
                "Sürüm {}  •  Windows Ultra Hızlı Spotlight & Launcher",
                APP_VERSION
            ),
            34,
            60,
            w - 32,
            85,
            DT_LEFT | DT_SINGLELINE,
        );

        // Separator line
        let sep_brush = CreateSolidBrush(COLOR_BORDER);
        let sep_rect = RECT {
            left: 32,
            top: 90,
            right: w - 32,
            bottom: 91,
        };
        FillRect(hdc, &sep_rect, sep_brush);
        let _ = DeleteObject(sep_brush.into());

        // Page Specific Content
        match &state.page {
            WizardPage::Welcome => {
                render_welcome_page(hdc, state, w, h, bold_font, body_font, title_font);
            }
            WizardPage::Options => {
                render_options_page(hdc, state, w, h, bold_font, body_font, title_font);
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
                    bold_font,
                    body_font,
                );
            }
            WizardPage::Completed { summary, is_repair } => {
                render_completed_page(
                    hdc, summary, *is_repair, state, w, h, bold_font, body_font, title_font,
                );
            }
            WizardPage::Uninstalled => {
                render_uninstalled_page(hdc, w, h, bold_font, body_font, title_font);
            }
        }

        SelectObject(hdc, old_font);
        let _ = DeleteObject(header_font.into());
        let _ = DeleteObject(body_font.into());
        let _ = DeleteObject(bold_font.into());
        let _ = DeleteObject(title_font.into());
    }
}

unsafe fn draw_button(
    hdc: HDC,
    rect: RECT,
    text: &str,
    is_hovered: bool,
    is_primary: bool,
    is_danger: bool,
) {
    let bg_col = if is_danger {
        if is_hovered {
            COLORREF(0x005555ff)
        } else {
            COLOR_DANGER
        }
    } else if is_primary {
        if is_hovered {
            COLOR_ACCENT_HOVER
        } else {
            COLOR_ACCENT
        }
    } else if is_hovered {
        COLORREF(0x00332826)
    } else {
        COLOR_CARD
    };

    unsafe {
        let brush = CreateSolidBrush(bg_col);
        FillRect(hdc, &rect, brush);
        let _ = DeleteObject(brush.into());

        let border_brush = CreateSolidBrush(if is_primary {
            COLOR_ACCENT
        } else {
            COLOR_BORDER
        });
        FrameRect(hdc, &rect, border_brush);
        let _ = DeleteObject(border_brush.into());

        SetTextColor(hdc, COLOR_TEXT);
    }
    draw_text(
        hdc,
        text,
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
}

unsafe fn draw_checkbox(hdc: HDC, x: i32, y: i32, checked: bool, label: &str, description: &str) {
    let box_rect = RECT {
        left: x,
        top: y + 2,
        right: x + 20,
        bottom: y + 22,
    };
    unsafe {
        let brush = CreateSolidBrush(if checked { COLOR_ACCENT } else { COLOR_CARD });
        FillRect(hdc, &box_rect, brush);
        let _ = DeleteObject(brush.into());

        let border_brush = CreateSolidBrush(if checked { COLOR_ACCENT } else { COLOR_BORDER });
        FrameRect(hdc, &box_rect, border_brush);
        let _ = DeleteObject(border_brush.into());

        if checked {
            SetTextColor(hdc, COLOR_TEXT);
            draw_text(
                hdc,
                "✓",
                box_rect.left,
                box_rect.top,
                box_rect.right,
                box_rect.bottom,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }

        SetTextColor(hdc, COLOR_TEXT);
    }
    draw_text(
        hdc,
        label,
        x + 30,
        y,
        x + 540,
        y + 22,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );

    if !description.is_empty() {
        unsafe {
            SetTextColor(hdc, COLOR_TEXT_MUTED);
        }
        draw_text(
            hdc,
            description,
            x + 30,
            y + 22,
            x + 540,
            y + 42,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
    }
}

// ---------------- Render Subpages ----------------

unsafe fn render_welcome_page(
    hdc: HDC,
    state: &WizardState,
    w: i32,
    _h: i32,
    _bold: HFONT,
    _body: HFONT,
    title_font: HFONT,
) {
    let card_rect = RECT {
        left: 32,
        top: 110,
        right: w - 32,
        bottom: 250,
    };
    unsafe {
        let card_brush = CreateSolidBrush(COLOR_CARD);
        FillRect(hdc, &card_rect, card_brush);
        let _ = DeleteObject(card_brush.into());
        let card_border = CreateSolidBrush(COLOR_BORDER);
        FrameRect(hdc, &card_rect, card_border);
        let _ = DeleteObject(card_border.into());

        SelectObject(hdc, title_font.into());
    }

    if state.installation_info.is_already_installed {
        // Installed Status Card
        unsafe {
            SetTextColor(hdc, COLOR_SUCCESS);
        }
        draw_text(
            hdc,
            "🟢 Bilgisayarınızda LiteCast Kurulu",
            52,
            125,
            w - 52,
            155,
            DT_LEFT | DT_SINGLELINE,
        );

        unsafe {
            SetTextColor(hdc, COLOR_TEXT);
        }
        draw_text(
            hdc,
            &format!(
                "📁 Konum: {}",
                state.installation_info.installed_path.display()
            ),
            52,
            160,
            w - 52,
            185,
            DT_LEFT | DT_SINGLELINE,
        );

        let hash_status = if state.installation_info.is_integrity_valid {
            "✅ Dosya bütünlüğü tam ve geçerli"
        } else {
            "⚠️ Dosya boyutu/hash farkı tespit edildi (Onarım önerilir)"
        };
        draw_text(
            hdc,
            &format!("🔒 Durum: {}", hash_status),
            52,
            190,
            w - 52,
            215,
            DT_LEFT | DT_SINGLELINE,
        );

        draw_text(
            hdc,
            &format!(
                "⚙️ Başlangıç: {}  •  Masaüstü Kısayolu: {}",
                if state.installation_info.autostart_enabled {
                    "Aktif"
                } else {
                    "Pasif"
                },
                if state.installation_info.desktop_shortcut_exists {
                    "Var"
                } else {
                    "Yok"
                }
            ),
            52,
            218,
            w - 52,
            240,
            DT_LEFT | DT_SINGLELINE,
        );

        // Action Buttons for existing install
        // Btn 0: Verify & Repair (Left)
        unsafe {
            draw_button(
                hdc,
                RECT {
                    left: 32,
                    top: 270,
                    right: 280,
                    bottom: 330,
                },
                "🔍 Dosyaları Doğrula & Onar",
                state.hovered_btn == Some(0),
                true,
                false,
            );
            draw_button(
                hdc,
                RECT {
                    left: 300,
                    top: 270,
                    right: w - 32,
                    bottom: 330,
                },
                "🔄 Yeniden Kur / Güncelle",
                state.hovered_btn == Some(1),
                false,
                false,
            );
            draw_button(
                hdc,
                RECT {
                    left: 32,
                    top: 350,
                    right: w - 32,
                    bottom: 400,
                },
                "🗑️ LiteCast'i Bilgisayardan Kaldır (Uninstall)",
                state.hovered_btn == Some(2),
                false,
                true,
            );
        }
    } else {
        // Not Installed Card
        unsafe {
            SetTextColor(hdc, COLOR_TEXT);
        }
        draw_text(
            hdc,
            "🚀 LiteCast Kurulumuna Hoş Geldiniz",
            52,
            130,
            w - 52,
            160,
            DT_LEFT | DT_SINGLELINE,
        );

        unsafe {
            SetTextColor(hdc, COLOR_TEXT_MUTED);
        }
        draw_text(
            hdc,
            "LiteCast, Windows için ultra hafif ve modern bir Spotlight/Raycast arama başlatıcısıdır.\n\
             Bu sihirbaz, gerekli dosyaları kuracak, kısayolları ve sistem entegrasyonlarını yapılandıracaktır.",
            52,
            168,
            w - 52,
            235,
            DT_LEFT | DT_WORDBREAK,
        );

        // Next Button
        unsafe {
            draw_button(
                hdc,
                RECT {
                    left: 32,
                    top: 300,
                    right: w - 32,
                    bottom: 360,
                },
                "Kurulum Seçeneklerine İlerle ➔",
                state.hovered_btn == Some(0),
                true,
                false,
            );
        }
    }
}

unsafe fn render_options_page(
    hdc: HDC,
    state: &WizardState,
    w: i32,
    _h: i32,
    _bold: HFONT,
    _body: HFONT,
    title_font: HFONT,
) {
    unsafe {
        SelectObject(hdc, title_font.into());
        SetTextColor(hdc, COLOR_TEXT);
    }
    draw_text(
        hdc,
        "⚙️ Kurulum Tercihleri",
        32,
        105,
        w - 32,
        130,
        DT_LEFT | DT_SINGLELINE,
    );

    // Install directory preview card
    let path_rect = RECT {
        left: 32,
        top: 135,
        right: w - 32,
        bottom: 195,
    };
    unsafe {
        let card_brush = CreateSolidBrush(COLOR_CARD);
        FillRect(hdc, &path_rect, card_brush);
        let _ = DeleteObject(card_brush.into());
        let card_border = CreateSolidBrush(COLOR_BORDER);
        FrameRect(hdc, &path_rect, card_border);
        let _ = DeleteObject(card_border.into());

        SetTextColor(hdc, COLOR_TEXT_MUTED);
    }
    draw_text(
        hdc,
        "Kurulum Klasörü (Varsayılan):",
        48,
        142,
        w - 48,
        162,
        DT_LEFT | DT_SINGLELINE,
    );

    unsafe {
        SetTextColor(hdc, COLOR_TEXT);
    }
    draw_text(
        hdc,
        &state.install_dir.to_string_lossy(),
        48,
        165,
        w - 48,
        188,
        DT_LEFT | DT_SINGLELINE,
    );

    // Checkboxes
    unsafe {
        draw_checkbox(
            hdc,
            36,
            210,
            state.opt_autostart,
            "Windows ile birlikte otomatik başlat",
            "Windows açıldığında LiteCast arka planda hazır bekler",
        );
        draw_checkbox(
            hdc,
            36,
            260,
            state.opt_desktop_shortcut,
            "Masaüstü kısayolu oluştur",
            "Masaüstünüze hızlı erişim simgesi ekler",
        );
        draw_checkbox(
            hdc,
            36,
            310,
            state.opt_startmenu_shortcut,
            "Başlat Menüsü kısayolu oluştur",
            "Windows arama ve Başlat menüsünden erişilebilir kılar",
        );

        // Bottom Navigation Buttons
        draw_button(
            hdc,
            RECT {
                left: 32,
                top: 380,
                right: 160,
                bottom: 430,
            },
            "← Geri",
            state.hovered_btn == Some(10),
            false,
            false,
        );
        draw_button(
            hdc,
            RECT {
                left: 180,
                top: 380,
                right: w - 32,
                bottom: 430,
            },
            "🚀 Kurulumu Başlat",
            state.hovered_btn == Some(11),
            true,
            false,
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
    _bold: HFONT,
    body_font: HFONT,
) {
    unsafe {
        SetTextColor(hdc, COLOR_TEXT);
    }
    draw_text(
        hdc,
        &format!("⏳ İşlem Yapılıyor: {}...", action_name),
        32,
        110,
        w - 32,
        140,
        DT_LEFT | DT_SINGLELINE,
    );

    // Progress Bar Background
    let bar_rect = RECT {
        left: 32,
        top: 150,
        right: w - 32,
        bottom: 170,
    };
    unsafe {
        let bar_bg = CreateSolidBrush(COLOR_CARD);
        FillRect(hdc, &bar_rect, bar_bg);
        let _ = DeleteObject(bar_bg.into());

        // Progress Fill
        let fill_w =
            ((w - 64) as f32 * (progress as f32 / 100.0)).clamp(0.0, (w - 64) as f32) as i32;
        if fill_w > 0 {
            let fill_rect = RECT {
                left: 32,
                top: 150,
                right: 32 + fill_w,
                bottom: 170,
            };
            let fill_bg = CreateSolidBrush(COLOR_ACCENT);
            FillRect(hdc, &fill_rect, fill_bg);
            let _ = DeleteObject(fill_bg.into());
        }

        let border_brush = CreateSolidBrush(COLOR_BORDER);
        FrameRect(hdc, &bar_rect, border_brush);
        let _ = DeleteObject(border_brush.into());

        // Log Console Box
        let log_box = RECT {
            left: 32,
            top: 190,
            right: w - 32,
            bottom: 410,
        };
        let log_bg = CreateSolidBrush(COLOR_CARD);
        FillRect(hdc, &log_box, log_bg);
        let _ = DeleteObject(log_bg.into());
        FrameRect(hdc, &log_box, border_brush);
        let _ = DeleteObject(border_brush.into());

        SelectObject(hdc, body_font.into());
        SetTextColor(hdc, COLOR_TEXT_MUTED);
    }

    let mut start_y = 205;
    for line in logs.iter().rev().take(7).rev() {
        draw_text(
            hdc,
            line,
            48,
            start_y,
            w - 48,
            start_y + 24,
            DT_LEFT | DT_SINGLELINE,
        );
        start_y += 26;
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
    _bold: HFONT,
    _body: HFONT,
    title_font: HFONT,
) {
    unsafe {
        SelectObject(hdc, title_font.into());
        SetTextColor(hdc, COLOR_SUCCESS);
    }

    let title_msg = if is_repair {
        "✅ Doğrulama ve Onarım Tamamlandı!"
    } else {
        "🎉 Kurulum Başarıyla Tamamlandı!"
    };
    draw_text(
        hdc,
        title_msg,
        32,
        115,
        w - 32,
        145,
        DT_LEFT | DT_SINGLELINE,
    );

    // Summary Card
    let card_rect = RECT {
        left: 32,
        top: 155,
        right: w - 32,
        bottom: 275,
    };
    unsafe {
        let card_brush = CreateSolidBrush(COLOR_CARD);
        FillRect(hdc, &card_rect, card_brush);
        let _ = DeleteObject(card_brush.into());
        let card_border = CreateSolidBrush(COLOR_BORDER);
        FrameRect(hdc, &card_rect, card_border);
        let _ = DeleteObject(card_border.into());

        SetTextColor(hdc, COLOR_TEXT);
    }
    draw_text(hdc, summary, 52, 170, w - 52, 260, DT_LEFT | DT_WORDBREAK);

    // Checkbox: Launch After Finish
    unsafe {
        draw_checkbox(
            hdc,
            36,
            300,
            state.opt_launch_after,
            "LiteCast'i Şimdi Başlat",
            "Kurulum sihirbazı kapandığında uygulamayı anında açar",
        );
        draw_button(
            hdc,
            RECT {
                left: 32,
                top: 370,
                right: w - 32,
                bottom: 430,
            },
            "Tamamla ve Kapat (Finish)",
            state.hovered_btn == Some(20),
            true,
            false,
        );
    }
}

unsafe fn render_uninstalled_page(
    hdc: HDC,
    w: i32,
    _h: i32,
    _bold: HFONT,
    _body: HFONT,
    title_font: HFONT,
) {
    unsafe {
        SelectObject(hdc, title_font.into());
        SetTextColor(hdc, COLOR_TEXT);
    }
    draw_text(
        hdc,
        "🗑️ LiteCast Sistemden Kaldırıldı",
        32,
        130,
        w - 32,
        165,
        DT_LEFT | DT_SINGLELINE,
    );

    unsafe {
        SetTextColor(hdc, COLOR_TEXT_MUTED);
    }
    draw_text(
        hdc,
        "Tüm program dosyaları, kısayollar ve Windows başlangıç kayıtları temizlendi.\n\
         LiteCast'i tercih ettiğiniz için teşekkür ederiz.",
        32,
        175,
        w - 32,
        250,
        DT_LEFT | DT_WORDBREAK,
    );

    unsafe {
        draw_button(
            hdc,
            RECT {
                left: 32,
                top: 320,
                right: w - 32,
                bottom: 380,
            },
            "Kapat",
            true,
            false,
            false,
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
        WM_KEYDOWN => {
            let vk = wparam.0 as u32;
            if vk == 0x1B {
                // ESC
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
    let mut need_repaint = false;
    let mut is_hand = false;

    {
        let mut guard = STATE.lock().unwrap();
        if let Some(state) = guard.as_mut() {
            let old_hover = state.hovered_btn;
            state.hovered_btn = None;

            match &state.page {
                WizardPage::Welcome => {
                    if state.installation_info.is_already_installed {
                        if (270..=330).contains(&y) {
                            if (32..=280).contains(&x) {
                                state.hovered_btn = Some(0);
                                is_hand = true;
                            } else if (300..=(WND_WIDTH - 32)).contains(&x) {
                                state.hovered_btn = Some(1);
                                is_hand = true;
                            }
                        } else if (350..=400).contains(&y) && (32..=(WND_WIDTH - 32)).contains(&x) {
                            state.hovered_btn = Some(2);
                            is_hand = true;
                        }
                    } else if (300..=360).contains(&y) && (32..=(WND_WIDTH - 32)).contains(&x) {
                        state.hovered_btn = Some(0);
                        is_hand = true;
                    }
                }
                WizardPage::Options => {
                    if (380..=430).contains(&y) {
                        if (32..=160).contains(&x) {
                            state.hovered_btn = Some(10);
                            is_hand = true;
                        } else if (180..=(WND_WIDTH - 32)).contains(&x) {
                            state.hovered_btn = Some(11);
                            is_hand = true;
                        }
                    } else if (210..=240).contains(&y)
                        || (260..=290).contains(&y)
                        || (310..=340).contains(&y)
                    {
                        is_hand = true;
                    }
                }
                WizardPage::Completed { .. } => {
                    if (370..=430).contains(&y) && (32..=(WND_WIDTH - 32)).contains(&x) {
                        state.hovered_btn = Some(20);
                        is_hand = true;
                    } else if (300..=330).contains(&y) {
                        is_hand = true;
                    }
                }
                WizardPage::Uninstalled
                    if (320..=380).contains(&y) && (32..=(WND_WIDTH - 32)).contains(&x) =>
                {
                    is_hand = true;
                }
                _ => {}
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
    let mut next_page: Option<WizardPage> = None;
    let mut should_close = false;
    let mut trigger_action: Option<&'static str> = None;

    {
        let mut guard = STATE.lock().unwrap();
        if let Some(state) = guard.as_mut() {
            match &state.page {
                WizardPage::Welcome => {
                    if state.installation_info.is_already_installed {
                        if (270..=330).contains(&y) {
                            if (32..=280).contains(&x) {
                                // Verify & Repair
                                trigger_action = Some("repair");
                            } else if (300..=(WND_WIDTH - 32)).contains(&x) {
                                // Reinstall / Options
                                next_page = Some(WizardPage::Options);
                            }
                        } else if (350..=400).contains(&y) && (32..=(WND_WIDTH - 32)).contains(&x) {
                            // Uninstall
                            trigger_action = Some("uninstall");
                        }
                    } else if (300..=360).contains(&y) && (32..=(WND_WIDTH - 32)).contains(&x) {
                        next_page = Some(WizardPage::Options);
                    }
                }
                WizardPage::Options => {
                    // Checkbox toggles
                    if (210..=240).contains(&y) && (36..=500).contains(&x) {
                        state.opt_autostart = !state.opt_autostart;
                    } else if (260..=290).contains(&y) && (36..=500).contains(&x) {
                        state.opt_desktop_shortcut = !state.opt_desktop_shortcut;
                    } else if (310..=340).contains(&y) && (36..=500).contains(&x) {
                        state.opt_startmenu_shortcut = !state.opt_startmenu_shortcut;
                    } else if (380..=430).contains(&y) {
                        if (32..=160).contains(&x) {
                            next_page = Some(WizardPage::Welcome);
                        } else if (180..=(WND_WIDTH - 32)).contains(&x) {
                            trigger_action = Some("install");
                        }
                    }
                }
                WizardPage::Completed { .. } => {
                    if (300..=330).contains(&y) && (36..=500).contains(&x) {
                        state.opt_launch_after = !state.opt_launch_after;
                    } else if (370..=430).contains(&y) && (32..=(WND_WIDTH - 32)).contains(&x) {
                        if state.opt_launch_after {
                            let bin = state.install_dir.join("LiteCast.exe");
                            start_installed_litecast(&bin);
                        }
                        should_close = true;
                    }
                }
                WizardPage::Uninstalled
                    if (320..=380).contains(&y) && (32..=(WND_WIDTH - 32)).contains(&x) =>
                {
                    should_close = true;
                }
                _ => {}
            }

            if let Some(np) = next_page {
                state.page = np;
            }
        }
    }

    if let Some(act) = trigger_action {
        match act {
            "install" => {
                let res = perform_install_or_repair(false);
                let mut guard = STATE.lock().unwrap();
                if let Some(state) = guard.as_mut() {
                    match res {
                        Ok(msg) => {
                            state.page = WizardPage::Completed {
                                summary: msg,
                                is_repair: false,
                            }
                        }
                        Err(e) => {
                            state.page = WizardPage::Completed {
                                summary: format!("Hata oluştu: {}", e),
                                is_repair: false,
                            }
                        }
                    }
                }
            }
            "repair" => {
                let res = perform_install_or_repair(true);
                let mut guard = STATE.lock().unwrap();
                if let Some(state) = guard.as_mut() {
                    match res {
                        Ok(msg) => {
                            state.page = WizardPage::Completed {
                                summary: msg,
                                is_repair: true,
                            }
                        }
                        Err(e) => {
                            state.page = WizardPage::Completed {
                                summary: format!("Onarım hatası: {}", e),
                                is_repair: true,
                            }
                        }
                    }
                }
            }
            "uninstall" => {
                let _ = perform_uninstall();
                let mut guard = STATE.lock().unwrap();
                if let Some(state) = guard.as_mut() {
                    state.page = WizardPage::Uninstalled;
                }
            }
            _ => {}
        }
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
    let info = detect_system_state();
    let default_dir = get_default_install_dir();

    {
        let mut guard = STATE.lock().unwrap();
        *guard = Some(WizardState {
            page: WizardPage::Welcome,
            install_dir: default_dir,
            opt_autostart: true,
            opt_desktop_shortcut: true,
            opt_startmenu_shortcut: true,
            opt_launch_after: true,
            installation_info: info,
            hovered_btn: None,
        });
    }

    let args: Vec<String> = std::env::args().collect();

    // CLI mode support for silent uninstall / verify
    if args.iter().any(|a| a == "--uninstall" || a == "-u") {
        let _ = perform_uninstall();
        return;
    }

    if args.iter().any(|a| a == "--verify" || a == "-v") {
        let _ = perform_install_or_repair(true);
        return;
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
