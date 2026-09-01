use std::sync::Mutex;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreateSolidBrush,
    DT_CENTER, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, DeleteDC,
    DeleteObject, DrawTextW, EndPaint, FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FONT_QUALITY,
    FillRect, HDC, HFONT, InvalidateRect, PAINTSTRUCT, SRCCOPY, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, DI_NORMAL, DefWindowProcW, DrawIconEx, HTCAPTION, IDC_ARROW, IDC_HAND,
    LoadCursorW, RegisterClassExW, SW_HIDE, SW_MINIMIZE, SW_RESTORE, SW_SHOW, SendMessageW,
    SetCursor, ShowWindow, WM_CLOSE, WM_CREATE, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_MOUSEMOVE,
    WM_NCLBUTTONDOWN, WM_PAINT, WNDCLASSEXW,
};
use windows::core::PCWSTR;

use crate::config::{AppConfig, OPACITY_PRESETS, THEME_PRESETS, rgb};
use crate::indexer::AppIndex;
use crate::platform_windows::{
    center_window_on_active_monitor, get_app_icon, is_autostart_enabled,
    register_configured_hotkey, set_autostart, set_window_icons, set_window_opacity, to_wide,
};
use crate::telemetry_local::{log_error, log_info};

pub const SETTINGS_WIDTH: i32 = 900;
pub const SETTINGS_HEIGHT: i32 = 620;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General = 0,
    Hotkeys = 1,
    Appearance = 2,
    Guide = 3,
}

pub struct SettingsState {
    pub config: AppConfig,
    pub index_count: usize,
    pub last_action_msg: String,
    pub hovered_btn: Option<usize>,
    pub active_tab: SettingsTab,
}

pub static SETTINGS_STATE: Mutex<Option<SettingsState>> = Mutex::new(None);
pub static SETTINGS_HWND: Mutex<Option<isize>> = Mutex::new(None);
pub static LAUNCHER_HWND: Mutex<Option<isize>> = Mutex::new(None);

pub fn init_settings_state(index_count: usize) {
    let mut state = SETTINGS_STATE.lock().unwrap();
    let config = AppConfig::load();
    *state = Some(SettingsState {
        config,
        index_count,
        last_action_msg: format!("Sistem hazır • {} uygulama indekslendi", index_count),
        hovered_btn: None,
        active_tab: SettingsTab::General,
    });
}

pub fn show_settings_window(hwnd: HWND) {
    unsafe {
        center_window_on_active_monitor(hwnd, SETTINGS_WIDTH, SETTINGS_HEIGHT);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = windows::Win32::UI::WindowsAndMessaging::BringWindowToTop(hwnd);
        let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

pub fn hide_settings_window(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

pub fn register_settings_class(hinstance: windows::Win32::Foundation::HINSTANCE) -> bool {
    let class_name = to_wide("LiteCastSettingsClass");
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or_default() };
    let icon = get_app_icon(64);
    let icon_sm = get_app_icon(32);

    let wnd_class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(settings_wnd_proc),
        hInstance: hinstance,
        hIcon: icon,
        hIconSm: icon_sm,
        hCursor: cursor,
        lpszClassName: PCWSTR::from_raw(class_name.as_ptr()),
        ..Default::default()
    };

    unsafe { RegisterClassExW(&wnd_class) != 0 }
}

/// Win32 Window Procedure for Settings Dashboard Window.
///
/// # Safety
///
/// Must be called by the Windows operating system with valid Win32 handles and messages.
pub unsafe extern "system" fn settings_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let mut guard = SETTINGS_HWND.lock().unwrap();
            *guard = Some(hwnd.0 as isize);
            set_window_icons(hwnd);
            LRESULT(0)
        }
        WM_PAINT => {
            unsafe {
                paint_settings_window(hwnd);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xffff) as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i32;
            handle_mouse_move(hwnd, x, y);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xffff) as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i32;
            handle_lbutton_down(hwnd, x, y);
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn handle_mouse_move(hwnd: HWND, x: i32, y: i32) {
    let mut rect = RECT::default();
    let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect) };
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    let btn_hit = get_button_at(x, y, width, height);
    let mut need_repaint = false;

    {
        let mut guard = SETTINGS_STATE.lock().unwrap();
        if let Some(state) = guard.as_mut()
            && state.hovered_btn != btn_hit
        {
            state.hovered_btn = btn_hit;
            need_repaint = true;
        }
    }

    if btn_hit.is_some() {
        unsafe {
            if let Ok(hand) = LoadCursorW(None, IDC_HAND) {
                let _ = SetCursor(Some(hand));
            }
        }
    }

    if need_repaint {
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }
}

fn get_button_at(x: i32, y: i32, width: i32, height: i32) -> Option<usize> {
    // 1. Top-Right Window Control Buttons (Minimize: 998, Close: 999)
    if (12..=42).contains(&y) {
        if ((width - 44)..=(width - 12)).contains(&x) {
            return Some(999); // Close (✕)
        }
        if ((width - 82)..=(width - 50)).contains(&x) {
            return Some(998); // Minimize (—)
        }
    }

    // 2. Sidebar Tab Buttons (x: 14..210)
    if (14..=210).contains(&x) {
        for i in 0..4 {
            let top = 96 + i * 48;
            let bot = top + 40;
            if (top..=bot).contains(&y) {
                return Some(10 + i as usize);
            }
        }

        // Live Service Status Badge in Sidebar Bottom
        if ((height - 54)..=(height - 18)).contains(&y) {
            return Some(14);
        }
    }

    let active_tab = {
        let guard = SETTINGS_STATE.lock().unwrap();
        guard
            .as_ref()
            .map(|s| s.active_tab)
            .unwrap_or(SettingsTab::General)
    };

    let start_x = 250;
    match active_tab {
        SettingsTab::General => {
            if (start_x..=(start_x + 195)).contains(&x) && (120..=165).contains(&y) {
                return Some(100);
            }
            if ((start_x + 207)..=(start_x + 402)).contains(&x) && (120..=165).contains(&y) {
                return Some(101);
            }
            if ((start_x + 414)..=(width - 30)).contains(&x) && (120..=165).contains(&y) {
                return Some(102);
            }
            if (start_x..=(width - 30)).contains(&x) && (220..=276).contains(&y) {
                return Some(300);
            }
        }
        SettingsTab::Hotkeys => {
            let chip_w = 135;
            let chip_gap = 14;
            for i in 0..4 {
                let left = start_x + i * (chip_w + chip_gap);
                let right = left + chip_w;
                if (left..=right).contains(&x) && (125..=170).contains(&y) {
                    return Some(200 + i as usize);
                }
            }
        }
        SettingsTab::Appearance => {
            let card_w = 135;
            let card_gap = 12;

            for i in 0..4 {
                let left = start_x + i * (card_w + card_gap);
                let right = left + card_w;
                if (left..=right).contains(&x) && (125..=175).contains(&y) {
                    return Some(400 + i as usize);
                }
            }

            for i in 4..7 {
                let left = start_x + (i - 4) * (card_w + card_gap);
                let right = left + card_w;
                if (left..=right).contains(&x) && (185..=235).contains(&y) {
                    return Some(400 + i as usize);
                }
            }

            let op_w = 108;
            let op_gap = 12;
            for (i, _) in OPACITY_PRESETS.iter().enumerate() {
                let left = start_x + (i as i32) * (op_w + op_gap);
                let right = left + op_w;
                if (left..=right).contains(&x) && (295..=340).contains(&y) {
                    return Some(500 + i);
                }
            }
        }
        SettingsTab::Guide => {}
    }

    None
}

fn handle_lbutton_down(hwnd: HWND, x: i32, y: i32) {
    let mut rect = RECT::default();
    let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect) };
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    let btn_hit = get_button_at(x, y, width, height);
    if let Some(btn) = btn_hit {
        match btn {
            998 => {
                // Minimize Window
                unsafe {
                    let _ = ShowWindow(hwnd, SW_MINIMIZE);
                }
                return;
            }
            999 => {
                // Hide / Close Window
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                return;
            }
            14 => {
                // Status card clicked: quick feedback
                let mut guard = SETTINGS_STATE.lock().unwrap();
                if let Some(state) = guard.as_mut() {
                    state.last_action_msg = format!(
                        "Sistem aktif ve çalışıyor • {} uygulama hazır",
                        state.index_count
                    );
                }
            }
            10..=13 => {
                let tab = match btn {
                    10 => SettingsTab::General,
                    11 => SettingsTab::Hotkeys,
                    12 => SettingsTab::Appearance,
                    13 => SettingsTab::Guide,
                    _ => SettingsTab::General,
                };
                let mut guard = SETTINGS_STATE.lock().unwrap();
                if let Some(state) = guard.as_mut() {
                    state.active_tab = tab;
                }
            }
            100 => {
                log_info("Settings: Opening launcher search window");
                let launcher_hwnd_opt = *LAUNCHER_HWND.lock().unwrap();
                if let Some(h_val) = launcher_hwnd_opt {
                    let h = HWND(h_val as *mut _);
                    crate::ui::show_window(h);
                }
            }
            101 => {
                log_info("Settings: Reindexing applications");
                let new_index = AppIndex::rebuild();
                let count = new_index.items.len();
                {
                    let mut guard = SETTINGS_STATE.lock().unwrap();
                    if let Some(state) = guard.as_mut() {
                        state.index_count = count;
                        state.last_action_msg =
                            format!("Veritabanı yenilendi: {} uygulama hazır", count);
                    }
                }
                crate::ui::update_launcher_index(new_index);
            }
            102 => {
                log_info("Settings: Checking for updates");
                let (update_url, hwnd_raw) = {
                    let mut guard = SETTINGS_STATE.lock().unwrap();
                    if let Some(state) = guard.as_mut() {
                        state.last_action_msg = "Güncellemeler denetleniyor...".into();
                        (state.config.update_url.clone(), hwnd.0 as isize)
                    } else {
                        (
                            crate::updater::DEFAULT_UPDATE_URL.to_string(),
                            hwnd.0 as isize,
                        )
                    }
                };
                std::thread::spawn(move || {
                    match crate::updater::check_for_updates(&update_url) {
                        Ok(Some(info)) => {
                            {
                                let mut guard = SETTINGS_STATE.lock().unwrap();
                                if let Some(state) = guard.as_mut() {
                                    state.last_action_msg =
                                        format!("✨ v{} bulundu, indiriliyor...", info.version);
                                }
                            }
                            let h = HWND(hwnd_raw as *mut _);
                            unsafe {
                                let _ = InvalidateRect(Some(h), None, false);
                            }
                            match crate::updater::download_and_apply_update(&info) {
                                Ok(_) => {
                                    let mut guard = SETTINGS_STATE.lock().unwrap();
                                    if let Some(state) = guard.as_mut() {
                                        state.last_action_msg = format!(
                                            "✨ v{} başarıyla indirildi! Yeniden başlatınca geçerli olacak.",
                                            info.version
                                        );
                                    }
                                }
                                Err(e) => {
                                    let mut guard = SETTINGS_STATE.lock().unwrap();
                                    if let Some(state) = guard.as_mut() {
                                        state.last_action_msg = format!("İndirme hatası: {}", e);
                                    }
                                }
                            }
                        }
                        Ok(None) => {
                            let mut guard = SETTINGS_STATE.lock().unwrap();
                            if let Some(state) = guard.as_mut() {
                                state.last_action_msg = format!(
                                    "LiteCast güncel (v{}).",
                                    crate::updater::CURRENT_VERSION
                                );
                            }
                        }
                        Err(e) => {
                            let mut guard = SETTINGS_STATE.lock().unwrap();
                            if let Some(state) = guard.as_mut() {
                                state.last_action_msg = format!("Güncelleme kontrolü: {}", e);
                            }
                        }
                    }
                    let h = HWND(hwnd_raw as *mut _);
                    unsafe {
                        let _ = InvalidateRect(Some(h), None, false);
                    }
                });
            }
            200..=203 => {
                let new_idx = btn - 200;
                let launcher_hwnd_opt = *LAUNCHER_HWND.lock().unwrap();
                let h = launcher_hwnd_opt.map(|v| HWND(v as *mut _));

                {
                    let mut guard = SETTINGS_STATE.lock().unwrap();
                    if let Some(state) = guard.as_mut() {
                        state.config.hotkey_index = new_idx;
                        state.config.save();
                        if let Some(lh) = h {
                            let ok = register_configured_hotkey(lh, new_idx);
                            state.last_action_msg = if ok {
                                format!("Kısayol güncellendi: {}", state.config.hotkey_name())
                            } else {
                                format!(
                                    "Kısayol güncellendi (Klavye kancası devrede): {}",
                                    state.config.hotkey_name()
                                )
                            };
                        }
                    }
                }
            }
            300 => {
                let cur = is_autostart_enabled();
                let next = !cur;
                let res = set_autostart(next);
                {
                    let mut guard = SETTINGS_STATE.lock().unwrap();
                    if let Some(state) = guard.as_mut() {
                        match res {
                            Ok(_) => {
                                state.config.autostart = next;
                                state.config.save();
                                state.last_action_msg = if next {
                                    "Başlangıç ayarı: Windows ile otomatik başlama AÇIK".into()
                                } else {
                                    "Başlangıç ayarı: Windows ile otomatik başlama KAPALI".into()
                                };
                            }
                            Err(e) => {
                                log_error(&format!("Autostart set failed: {}", e));
                                state.last_action_msg = format!("Ayar hatası: {}", e);
                            }
                        }
                    }
                }
            }
            400..=406 => {
                let theme_idx = btn - 400;
                let mut guard = SETTINGS_STATE.lock().unwrap();
                if let Some(state) = guard.as_mut() {
                    state.config.theme_index = theme_idx;
                    state.config.save();
                    state.last_action_msg = format!(
                        "Tema güncellendi: {}",
                        THEME_PRESETS
                            .get(theme_idx)
                            .map(|t| t.name)
                            .unwrap_or("Özel")
                    );
                }
                if let Some(lh_val) = *LAUNCHER_HWND.lock().unwrap() {
                    let lh = HWND(lh_val as *mut _);
                    unsafe {
                        let _ = InvalidateRect(Some(lh), None, false);
                    }
                }
            }
            500..=504 => {
                let op_idx = btn - 500;
                if let Some(&op_val) = OPACITY_PRESETS.get(op_idx) {
                    let mut guard = SETTINGS_STATE.lock().unwrap();
                    if let Some(state) = guard.as_mut() {
                        state.config.opacity_percent = op_val;
                        state.config.save();
                        state.last_action_msg = format!("Arama çubuğu saydamlığı: %{}", op_val);
                    }
                    if let Some(lh_val) = *LAUNCHER_HWND.lock().unwrap() {
                        let lh = HWND(lh_val as *mut _);
                        set_window_opacity(lh, op_val);
                        unsafe {
                            let _ = InvalidateRect(Some(lh), None, false);
                        }
                    }
                }
            }
            _ => {}
        }

        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    } else {
        // Drag window if clicked on header / top title area
        if y <= 50 || (x <= 225 && y <= 85) {
            unsafe {
                let _ = ReleaseCapture();
                let _ = SendMessageW(
                    hwnd,
                    WM_NCLBUTTONDOWN,
                    Some(WPARAM(HTCAPTION as usize)),
                    Some(LPARAM(0)),
                );
            }
        }
    }
}

unsafe fn paint_settings_window(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };

    let mut rect = RECT::default();
    let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect) };
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    let mem_dc = unsafe { CreateCompatibleDC(Some(hdc)) };
    let mem_bitmap = unsafe { CreateCompatibleBitmap(hdc, width, height) };
    let old_bmp = unsafe { SelectObject(mem_dc, mem_bitmap.into()) };

    // 1. Outer Background fill (Obsidian)
    let bg_color = rgb(18, 18, 22);
    let brush = unsafe { CreateSolidBrush(bg_color) };
    unsafe {
        FillRect(mem_dc, &rect, brush);
        let _ = DeleteObject(brush.into());
        SetBkMode(mem_dc, TRANSPARENT);
    }

    // 2. Outer Subtle Border (1px)
    let border_brush = unsafe { CreateSolidBrush(rgb(48, 48, 56)) };
    let border_rect_top = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: 1,
    };
    let border_rect_bottom = RECT {
        left: 0,
        top: height - 1,
        right: width,
        bottom: height,
    };
    let border_rect_left = RECT {
        left: 0,
        top: 0,
        right: 1,
        bottom: height,
    };
    let border_rect_right = RECT {
        left: width - 1,
        top: 0,
        right: width,
        bottom: height,
    };
    unsafe {
        FillRect(mem_dc, &border_rect_top, border_brush);
        FillRect(mem_dc, &border_rect_bottom, border_brush);
        FillRect(mem_dc, &border_rect_left, border_brush);
        FillRect(mem_dc, &border_rect_right, border_brush);
        let _ = DeleteObject(border_brush.into());
    }

    let state_guard = SETTINGS_STATE.lock().unwrap();
    if let Some(state) = state_guard.as_ref() {
        let theme = state.config.theme();

        // 3. Sidebar (0..225)
        let sidebar_rect = RECT {
            left: 0,
            top: 0,
            right: 225,
            bottom: height,
        };
        let sidebar_brush = unsafe { CreateSolidBrush(rgb(24, 24, 28)) };
        unsafe {
            FillRect(mem_dc, &sidebar_rect, sidebar_brush);
            let _ = DeleteObject(sidebar_brush.into());
        }

        // Sidebar Divider line
        let div_brush = unsafe { CreateSolidBrush(rgb(39, 39, 45)) };
        let div_rect = RECT {
            left: 224,
            top: 0,
            right: 225,
            bottom: height,
        };
        unsafe {
            FillRect(mem_dc, &div_rect, div_brush);
            let _ = DeleteObject(div_brush.into());
        }

        let hfont_brand = create_font(20, 700);
        let hfont_small = create_font(12, 400);
        let hfont_tab = create_font(14, 600);
        let hfont_title = create_font(20, 700);
        let hfont_section = create_font(15, 600);
        let hfont_body = create_font(13, 400);
        let hfont_bold = create_font(13, 600);
        let hfont_ctrl = create_font(15, 600);

        // Sidebar Brand Header
        let app_icon = get_app_icon(32);
        unsafe {
            let _ = DrawIconEx(mem_dc, 20, 22, app_icon, 26, 26, 0, None, DI_NORMAL);
            let _ = SelectObject(mem_dc, hfont_brand.into());
            SetTextColor(mem_dc, rgb(255, 255, 255));
        }
        draw_text_str(mem_dc, "LiteCast", 54, 22, 210, 48, DT_LEFT | DT_SINGLELINE);

        unsafe {
            let _ = SelectObject(mem_dc, hfont_small.into());
            SetTextColor(mem_dc, rgb(161, 161, 170));
        }
        draw_text_str(
            mem_dc,
            "v1.0.0 Kontrol Paneli",
            54,
            46,
            210,
            68,
            DT_LEFT | DT_SINGLELINE,
        );

        // Under-brand divider line in sidebar
        let sep_brush = unsafe { CreateSolidBrush(rgb(36, 36, 42)) };
        let sep_rect = RECT {
            left: 18,
            top: 78,
            right: 206,
            bottom: 79,
        };
        unsafe {
            FillRect(mem_dc, &sep_rect, sep_brush);
            let _ = DeleteObject(sep_brush.into());
        }

        // Sidebar Tabs
        let tabs = [
            (SettingsTab::General, "⚙️  Genel Ayarlar", 10),
            (SettingsTab::Hotkeys, "⌨️  Kısayol & Aliases", 11),
            (SettingsTab::Appearance, "🎨  Görünüm & Tema", 12),
            (SettingsTab::Guide, "📖  Kılavuz & İpuçları", 13),
        ];

        for (i, &(tab_type, tab_label, btn_id)) in tabs.iter().enumerate() {
            let is_active = state.active_tab == tab_type;
            let is_hover = state.hovered_btn == Some(btn_id);

            let top = 96 + (i as i32) * 48;
            let tab_rect = RECT {
                left: 14,
                top,
                right: 210,
                bottom: top + 40,
            };

            if is_active {
                draw_rounded_card(mem_dc, &tab_rect, theme.primary, theme.primary_border);
                unsafe {
                    SetTextColor(mem_dc, theme.text_on_primary);
                    let _ = SelectObject(mem_dc, hfont_tab.into());
                }
            } else if is_hover {
                draw_rounded_card(mem_dc, &tab_rect, rgb(38, 38, 46), rgb(60, 60, 70));
                unsafe {
                    SetTextColor(mem_dc, rgb(255, 255, 255));
                    let _ = SelectObject(mem_dc, hfont_tab.into());
                }
            } else {
                unsafe {
                    SetTextColor(mem_dc, rgb(161, 161, 170));
                    let _ = SelectObject(mem_dc, hfont_tab.into());
                }
            }

            draw_text_str(
                mem_dc,
                tab_label,
                26,
                top,
                205,
                top + 40,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );
        }

        // Live Service Status Badge in Sidebar Bottom (Guaranteed visible and bounded)
        let is_status_hover = state.hovered_btn == Some(14);
        let status_rect = RECT {
            left: 14,
            top: height - 54,
            right: 210,
            bottom: height - 18,
        };
        let status_bg = if is_status_hover {
            rgb(22, 48, 32)
        } else {
            rgb(14, 30, 22)
        };
        draw_rounded_card(mem_dc, &status_rect, status_bg, rgb(34, 197, 94));
        unsafe {
            SetTextColor(mem_dc, rgb(34, 197, 94));
            let _ = SelectObject(mem_dc, hfont_bold.into());
        }
        draw_text_str(
            mem_dc,
            "🟢 Servis Aktif & Hazır",
            14,
            height - 54,
            210,
            height - 18,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        // 4. Custom Header Window Control Buttons (Top-Right: Minimize and Close)
        // Minimize Button (998)
        let min_hover = state.hovered_btn == Some(998);
        let min_rect = RECT {
            left: width - 82,
            top: 12,
            right: width - 50,
            bottom: 42,
        };
        let min_bg = if min_hover {
            rgb(42, 42, 50)
        } else {
            rgb(26, 26, 32)
        };
        let min_border = if min_hover {
            rgb(70, 70, 80)
        } else {
            rgb(44, 44, 52)
        };
        draw_rounded_card(mem_dc, &min_rect, min_bg, min_border);
        unsafe {
            let _ = SelectObject(mem_dc, hfont_ctrl.into());
            SetTextColor(
                mem_dc,
                if min_hover {
                    rgb(255, 255, 255)
                } else {
                    rgb(180, 180, 190)
                },
            );
        }
        draw_text_str(
            mem_dc,
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
            left: width - 44,
            top: 12,
            right: width - 12,
            bottom: 42,
        };
        let close_bg = if close_hover {
            rgb(220, 38, 38)
        } else {
            rgb(26, 26, 32)
        };
        let close_border = if close_hover {
            rgb(239, 68, 68)
        } else {
            rgb(44, 44, 52)
        };
        draw_rounded_card(mem_dc, &close_rect, close_bg, close_border);
        unsafe {
            let _ = SelectObject(mem_dc, hfont_ctrl.into());
            SetTextColor(
                mem_dc,
                if close_hover {
                    rgb(255, 255, 255)
                } else {
                    rgb(180, 180, 190)
                },
            );
        }
        draw_text_str(
            mem_dc,
            "✕",
            close_rect.left,
            close_rect.top,
            close_rect.right,
            close_rect.bottom,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        // 5. Right Content Panel
        let content_x = 250;
        match state.active_tab {
            SettingsTab::General => {
                unsafe {
                    let _ = SelectObject(mem_dc, hfont_title.into());
                    SetTextColor(mem_dc, rgb(255, 255, 255));
                }
                draw_text_str(
                    mem_dc,
                    "Genel Ayarlar & Eylemler",
                    content_x,
                    22,
                    width - 100,
                    48,
                    DT_LEFT | DT_SINGLELINE,
                );

                unsafe {
                    let _ = SelectObject(mem_dc, hfont_body.into());
                    SetTextColor(mem_dc, rgb(161, 161, 170));
                }
                draw_text_str(
                    mem_dc,
                    "Uygulama başlatıcıyı kontrol edin ve sistem entegrasyonunu yönetin.",
                    content_x,
                    50,
                    width - 100,
                    72,
                    DT_LEFT | DT_SINGLELINE,
                );

                // Section 1: Hızlı Eylemler
                unsafe {
                    let _ = SelectObject(mem_dc, hfont_section.into());
                    SetTextColor(mem_dc, rgb(244, 244, 245));
                }
                draw_text_str(
                    mem_dc,
                    "HIZLI EYLEMLER",
                    content_x,
                    90,
                    800,
                    114,
                    DT_LEFT | DT_SINGLELINE,
                );

                let btn1_rect = RECT {
                    left: 250,
                    top: 120,
                    right: 445,
                    bottom: 165,
                };
                let btn1_hover = state.hovered_btn == Some(100);
                let btn1_bg = if btn1_hover {
                    theme.primary_hover
                } else {
                    theme.primary
                };
                draw_rounded_card(mem_dc, &btn1_rect, btn1_bg, theme.primary_border);
                unsafe {
                    SetTextColor(mem_dc, theme.text_on_primary);
                    let _ = SelectObject(mem_dc, hfont_bold.into());
                }
                draw_text_str(
                    mem_dc,
                    "🔍  Arama Çubuğu",
                    btn1_rect.left,
                    btn1_rect.top,
                    btn1_rect.right,
                    btn1_rect.bottom,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );

                let btn2_rect = RECT {
                    left: 457,
                    top: 120,
                    right: 652,
                    bottom: 165,
                };
                let btn2_hover = state.hovered_btn == Some(101);
                let btn2_bg = if btn2_hover {
                    rgb(45, 45, 52)
                } else {
                    rgb(32, 32, 38)
                };
                draw_rounded_card(mem_dc, &btn2_rect, btn2_bg, rgb(63, 63, 70));
                unsafe {
                    SetTextColor(mem_dc, rgb(228, 228, 231));
                    let _ = SelectObject(mem_dc, hfont_bold.into());
                }
                draw_text_str(
                    mem_dc,
                    "🔄  İndeksi Yenile",
                    btn2_rect.left,
                    btn2_rect.top,
                    btn2_rect.right,
                    btn2_rect.bottom,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );

                let btn3_rect = RECT {
                    left: 664,
                    top: 120,
                    right: width - 30,
                    bottom: 165,
                };
                let btn3_hover = state.hovered_btn == Some(102);
                let btn3_bg = if btn3_hover {
                    rgb(45, 45, 52)
                } else {
                    rgb(32, 32, 38)
                };
                draw_rounded_card(mem_dc, &btn3_rect, btn3_bg, rgb(63, 63, 70));
                unsafe {
                    SetTextColor(mem_dc, rgb(228, 228, 231));
                    let _ = SelectObject(mem_dc, hfont_bold.into());
                }
                draw_text_str(
                    mem_dc,
                    "✨  Güncellemeleri Denetle",
                    btn3_rect.left,
                    btn3_rect.top,
                    btn3_rect.right,
                    btn3_rect.bottom,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );

                // Section 2: Windows Başlangıç
                unsafe {
                    let _ = SelectObject(mem_dc, hfont_section.into());
                    SetTextColor(mem_dc, rgb(244, 244, 245));
                }
                draw_text_str(
                    mem_dc,
                    "WINDOWS ENTEGRASYONU",
                    content_x,
                    190,
                    800,
                    212,
                    DT_LEFT | DT_SINGLELINE,
                );

                let auto_on = is_autostart_enabled();
                let auto_card = RECT {
                    left: 250,
                    top: 220,
                    right: width - 30,
                    bottom: 276,
                };
                let auto_hover = state.hovered_btn == Some(300);
                let auto_bg = if auto_hover {
                    rgb(36, 36, 44)
                } else {
                    rgb(26, 26, 32)
                };
                let auto_border = if auto_on {
                    theme.primary
                } else {
                    rgb(50, 50, 58)
                };
                draw_rounded_card(mem_dc, &auto_card, auto_bg, auto_border);

                unsafe {
                    let _ = SelectObject(mem_dc, hfont_bold.into());
                    SetTextColor(
                        mem_dc,
                        if auto_on {
                            rgb(255, 255, 255)
                        } else {
                            rgb(212, 212, 216)
                        },
                    );
                }
                let check_str = if auto_on {
                    "🚀  Windows başladığında LiteCast otomatik çalışsın (Arka planda hazır bekler)"
                } else {
                    "🚀  Windows başladığında LiteCast otomatik çalışsın (Şu an kapalı)"
                };
                draw_text_str(
                    mem_dc,
                    check_str,
                    270,
                    220,
                    width - 130,
                    276,
                    DT_LEFT | DT_VCENTER | DT_SINGLELINE,
                );

                let badge_auto = RECT {
                    left: width - 120,
                    top: 230,
                    right: width - 42,
                    bottom: 266,
                };
                if auto_on {
                    draw_rounded_card(mem_dc, &badge_auto, rgb(16, 80, 48), rgb(34, 197, 94));
                    unsafe {
                        SetTextColor(mem_dc, rgb(34, 197, 94));
                        let _ = SelectObject(mem_dc, hfont_small.into());
                    }
                    draw_text_str(
                        mem_dc,
                        "AÇIK",
                        badge_auto.left,
                        badge_auto.top,
                        badge_auto.right,
                        badge_auto.bottom,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                } else {
                    draw_rounded_card(mem_dc, &badge_auto, rgb(40, 40, 46), rgb(80, 80, 90));
                    unsafe {
                        SetTextColor(mem_dc, rgb(161, 161, 170));
                        let _ = SelectObject(mem_dc, hfont_small.into());
                    }
                    draw_text_str(
                        mem_dc,
                        "KAPALI",
                        badge_auto.left,
                        badge_auto.top,
                        badge_auto.right,
                        badge_auto.bottom,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                }

                // Section 3: Sistem Özeti
                let summary_card = RECT {
                    left: 250,
                    top: 300,
                    right: width - 30,
                    bottom: height - 60,
                };
                draw_rounded_card(mem_dc, &summary_card, rgb(24, 24, 30), rgb(39, 39, 48));

                unsafe {
                    let _ = SelectObject(mem_dc, hfont_section.into());
                    SetTextColor(mem_dc, theme.primary_light);
                }
                draw_text_str(
                    mem_dc,
                    "📊  SİSTEM VE ÇALIŞMA ZAMANI ÖZETİ",
                    270,
                    315,
                    width - 50,
                    340,
                    DT_LEFT | DT_SINGLELINE,
                );

                unsafe {
                    let _ = SelectObject(mem_dc, hfont_body.into());
                    SetTextColor(mem_dc, rgb(212, 212, 216));
                }
                let stat_1 = format!(
                    "• Toplam İndekslenen Uygulama & Oyun: {} adet (0ms anında arama)",
                    state.index_count
                );
                let stat_2 = format!(
                    "• Global Tetikleme Kısayolu: {} (Düşük gecikmeli klavye kancası devrede)",
                    state.config.hotkey_name()
                );
                let stat_3 = format!(
                    "• Güncel Sürüm: v{} | Otomatik Güncelleme: {}",
                    crate::updater::CURRENT_VERSION,
                    if state.config.auto_update {
                        "Açık"
                    } else {
                        "Kapalı"
                    }
                );
                let stat_4 = format!(
                    "• Seçili Görünüm Teması: {} | Saydamlık: %{}",
                    theme.name, state.config.opacity_percent
                );
                let stat_full = format!("{}\n{}\n{}\n{}", stat_1, stat_2, stat_3, stat_4);
                draw_text_str(
                    mem_dc,
                    &stat_full,
                    270,
                    348,
                    width - 50,
                    height - 70,
                    DT_LEFT | DT_WORDBREAK,
                );
            }
            SettingsTab::Hotkeys => {
                unsafe {
                    let _ = SelectObject(mem_dc, hfont_title.into());
                    SetTextColor(mem_dc, rgb(255, 255, 255));
                }
                draw_text_str(
                    mem_dc,
                    "Kısayol Tuşları & Takma Adlar (Aliases)",
                    content_x,
                    22,
                    width - 100,
                    48,
                    DT_LEFT | DT_SINGLELINE,
                );

                unsafe {
                    let _ = SelectObject(mem_dc, hfont_body.into());
                    SetTextColor(mem_dc, rgb(161, 161, 170));
                }
                draw_text_str(
                    mem_dc,
                    "Arama çubuğunu açma kısayolunu ve hızlı arama takma adlarını özelleştirin.",
                    content_x,
                    50,
                    width - 100,
                    72,
                    DT_LEFT | DT_SINGLELINE,
                );

                // Section 1: Global Hotkey
                unsafe {
                    let _ = SelectObject(mem_dc, hfont_section.into());
                    SetTextColor(mem_dc, rgb(244, 244, 245));
                }
                draw_text_str(
                    mem_dc,
                    "GLOBAL ARAMA KISAYOLU",
                    content_x,
                    95,
                    800,
                    118,
                    DT_LEFT | DT_SINGLELINE,
                );

                let hotkey_labels = ["Alt + Space", "Ctrl + Space", "Ctrl + K", "F2"];
                let chip_w = 135;
                let chip_gap = 14;
                for (i, &label) in hotkey_labels.iter().enumerate() {
                    let is_active = state.config.hotkey_index == i;
                    let is_hover = state.hovered_btn == Some(200 + i);

                    let left = content_x + (i as i32) * (chip_w + chip_gap);
                    let chip_rect = RECT {
                        left,
                        top: 125,
                        right: left + chip_w,
                        bottom: 170,
                    };

                    if is_active {
                        draw_rounded_card(mem_dc, &chip_rect, theme.primary, theme.primary_border);
                        unsafe {
                            SetTextColor(mem_dc, theme.text_on_primary);
                            let _ = SelectObject(mem_dc, hfont_bold.into());
                        }
                    } else if is_hover {
                        draw_rounded_card(mem_dc, &chip_rect, rgb(45, 45, 52), rgb(75, 75, 85));
                        unsafe {
                            SetTextColor(mem_dc, rgb(255, 255, 255));
                            let _ = SelectObject(mem_dc, hfont_bold.into());
                        }
                    } else {
                        draw_rounded_card(mem_dc, &chip_rect, rgb(28, 28, 34), rgb(48, 48, 56));
                        unsafe {
                            SetTextColor(mem_dc, rgb(212, 212, 216));
                            let _ = SelectObject(mem_dc, hfont_bold.into());
                        }
                    }

                    draw_text_str(
                        mem_dc,
                        label,
                        left,
                        125,
                        left + chip_w,
                        170,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                }

                // Section 2: Hızlı Kısayollar
                let aliases_card = RECT {
                    left: 250,
                    top: 195,
                    right: width - 30,
                    bottom: height - 60,
                };
                draw_rounded_card(mem_dc, &aliases_card, rgb(24, 24, 30), rgb(39, 39, 48));

                unsafe {
                    let _ = SelectObject(mem_dc, hfont_section.into());
                    SetTextColor(mem_dc, theme.primary_light);
                }
                draw_text_str(
                    mem_dc,
                    "⚡  HIZLI TAKMA ADLAR (ALIASES TABLOSU)",
                    270,
                    210,
                    width - 50,
                    235,
                    DT_LEFT | DT_SINGLELINE,
                );

                unsafe {
                    let _ = SelectObject(mem_dc, hfont_body.into());
                    SetTextColor(mem_dc, rgb(161, 161, 170));
                }
                draw_text_str(
                    mem_dc,
                    "Arama çubuğuna doğrudan bu kısaltmaları yazarak hızlıca başlatabilirsiniz:",
                    270,
                    238,
                    width - 50,
                    260,
                    DT_LEFT | DT_SINGLELINE,
                );

                let aliases_rows = [
                    ("yt", "YouTube", "sp", "Spotify", "st", "Steam"),
                    ("dc", "Discord", "wa", "WhatsApp", "gh", "GitHub"),
                    (
                        "cb",
                        "Pano Geçmişi",
                        ">",
                        "Terminal Koşturucu",
                        "Tab",
                        "Eylemler Menüsü",
                    ),
                    (
                        "json",
                        "JSON Biçimlendir",
                        "jwt",
                        "JWT Çözümle",
                        "calc",
                        "Hesap Makinesi",
                    ),
                    (
                        "ps",
                        "PowerShell",
                        "cmd",
                        "Komut İstemi",
                        "help",
                        "Kılavuz & Yardım",
                    ),
                ];

                let mut cur_y = 275;
                for row in aliases_rows {
                    unsafe {
                        let _ = SelectObject(mem_dc, hfont_bold.into());
                        SetTextColor(mem_dc, theme.primary_light);
                    }
                    draw_text_str(
                        mem_dc,
                        row.0,
                        275,
                        cur_y,
                        330,
                        cur_y + 24,
                        DT_LEFT | DT_SINGLELINE,
                    );
                    unsafe {
                        let _ = SelectObject(mem_dc, hfont_body.into());
                        SetTextColor(mem_dc, rgb(228, 228, 231));
                    }
                    draw_text_str(
                        mem_dc,
                        &format!("➔  {}", row.1),
                        325,
                        cur_y,
                        460,
                        cur_y + 24,
                        DT_LEFT | DT_SINGLELINE,
                    );

                    unsafe {
                        let _ = SelectObject(mem_dc, hfont_bold.into());
                        SetTextColor(mem_dc, theme.primary_light);
                    }
                    draw_text_str(
                        mem_dc,
                        row.2,
                        470,
                        cur_y,
                        525,
                        cur_y + 24,
                        DT_LEFT | DT_SINGLELINE,
                    );
                    unsafe {
                        let _ = SelectObject(mem_dc, hfont_body.into());
                        SetTextColor(mem_dc, rgb(228, 228, 231));
                    }
                    draw_text_str(
                        mem_dc,
                        &format!("➔  {}", row.3),
                        520,
                        cur_y,
                        655,
                        cur_y + 24,
                        DT_LEFT | DT_SINGLELINE,
                    );

                    unsafe {
                        let _ = SelectObject(mem_dc, hfont_bold.into());
                        SetTextColor(mem_dc, theme.primary_light);
                    }
                    draw_text_str(
                        mem_dc,
                        row.4,
                        665,
                        cur_y,
                        720,
                        cur_y + 24,
                        DT_LEFT | DT_SINGLELINE,
                    );
                    unsafe {
                        let _ = SelectObject(mem_dc, hfont_body.into());
                        SetTextColor(mem_dc, rgb(228, 228, 231));
                    }
                    draw_text_str(
                        mem_dc,
                        &format!("➔  {}", row.5),
                        715,
                        cur_y,
                        width - 40,
                        cur_y + 24,
                        DT_LEFT | DT_SINGLELINE,
                    );

                    cur_y += 34;
                }
            }
            SettingsTab::Appearance => {
                unsafe {
                    let _ = SelectObject(mem_dc, hfont_title.into());
                    SetTextColor(mem_dc, rgb(255, 255, 255));
                }
                draw_text_str(
                    mem_dc,
                    "Görünüm & Tema Kişiselleştirme",
                    content_x,
                    22,
                    width - 100,
                    48,
                    DT_LEFT | DT_SINGLELINE,
                );

                unsafe {
                    let _ = SelectObject(mem_dc, hfont_body.into());
                    SetTextColor(mem_dc, rgb(161, 161, 170));
                }
                draw_text_str(
                    mem_dc,
                    "Arayüz tema renklerini ve arama çubuğu saydamlığını özelleştirin.",
                    content_x,
                    50,
                    width - 100,
                    72,
                    DT_LEFT | DT_SINGLELINE,
                );

                // Section 1: Tema Renkleri
                unsafe {
                    let _ = SelectObject(mem_dc, hfont_section.into());
                    SetTextColor(mem_dc, rgb(244, 244, 245));
                }
                draw_text_str(
                    mem_dc,
                    "VURGU TEMA RENKLERİ",
                    content_x,
                    90,
                    800,
                    114,
                    DT_LEFT | DT_SINGLELINE,
                );

                let card_w = 135;
                let card_gap = 12;

                for (i, t) in THEME_PRESETS.iter().enumerate().take(4) {
                    let is_active = state.config.theme_index == i;
                    let is_hover = state.hovered_btn == Some(400 + i);

                    let left = content_x + (i as i32) * (card_w + card_gap);
                    let card_rect = RECT {
                        left,
                        top: 125,
                        right: left + card_w,
                        bottom: 175,
                    };

                    let bg = if is_active {
                        t.primary
                    } else if is_hover {
                        rgb(40, 40, 48)
                    } else {
                        rgb(26, 26, 32)
                    };
                    let border = if is_active {
                        t.primary_border
                    } else {
                        rgb(52, 52, 60)
                    };
                    draw_rounded_card(mem_dc, &card_rect, bg, border);

                    unsafe {
                        let _ = SelectObject(mem_dc, hfont_bold.into());
                        SetTextColor(
                            mem_dc,
                            if is_active {
                                t.text_on_primary
                            } else {
                                rgb(228, 228, 231)
                            },
                        );
                    }
                    draw_text_str(
                        mem_dc,
                        t.name,
                        left,
                        125,
                        left + card_w,
                        175,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                }

                for (i, t) in THEME_PRESETS.iter().enumerate().skip(4) {
                    let is_active = state.config.theme_index == i;
                    let is_hover = state.hovered_btn == Some(400 + i);

                    let left = content_x + ((i - 4) as i32) * (card_w + card_gap);
                    let card_rect = RECT {
                        left,
                        top: 185,
                        right: left + card_w,
                        bottom: 235,
                    };

                    let bg = if is_active {
                        t.primary
                    } else if is_hover {
                        rgb(40, 40, 48)
                    } else {
                        rgb(26, 26, 32)
                    };
                    let border = if is_active {
                        t.primary_border
                    } else {
                        rgb(52, 52, 60)
                    };
                    draw_rounded_card(mem_dc, &card_rect, bg, border);

                    unsafe {
                        let _ = SelectObject(mem_dc, hfont_bold.into());
                        SetTextColor(
                            mem_dc,
                            if is_active {
                                t.text_on_primary
                            } else {
                                rgb(228, 228, 231)
                            },
                        );
                    }
                    draw_text_str(
                        mem_dc,
                        t.name,
                        left,
                        185,
                        left + card_w,
                        235,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                }

                // Section 2: Saydamlık
                unsafe {
                    let _ = SelectObject(mem_dc, hfont_section.into());
                    SetTextColor(mem_dc, rgb(244, 244, 245));
                }
                draw_text_str(
                    mem_dc,
                    "ARAMA ÇUBUĞU SAYDAMLIK DERECESİ",
                    content_x,
                    260,
                    800,
                    285,
                    DT_LEFT | DT_SINGLELINE,
                );

                let op_labels = [
                    "%100 (Opak)",
                    "%95 (Önerilen)",
                    "%90 (Dengeli)",
                    "%85 (Hafif)",
                    "%75 (Saydam)",
                ];
                let op_w = 108;
                let op_gap = 12;
                for (i, (&op_val, &op_label)) in
                    OPACITY_PRESETS.iter().zip(op_labels.iter()).enumerate()
                {
                    let is_active = state.config.opacity_percent == op_val;
                    let is_hover = state.hovered_btn == Some(500 + i);

                    let left = content_x + (i as i32) * (op_w + op_gap);
                    let op_rect = RECT {
                        left,
                        top: 295,
                        right: left + op_w,
                        bottom: 340,
                    };

                    let bg = if is_active {
                        theme.primary
                    } else if is_hover {
                        rgb(40, 40, 48)
                    } else {
                        rgb(26, 26, 32)
                    };
                    let border = if is_active {
                        theme.primary_border
                    } else {
                        rgb(52, 52, 60)
                    };
                    draw_rounded_card(mem_dc, &op_rect, bg, border);

                    unsafe {
                        let _ = SelectObject(mem_dc, hfont_bold.into());
                        SetTextColor(
                            mem_dc,
                            if is_active {
                                theme.text_on_primary
                            } else {
                                rgb(228, 228, 231)
                            },
                        );
                    }
                    draw_text_str(
                        mem_dc,
                        op_label,
                        left,
                        295,
                        left + op_w,
                        340,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                }

                // Live Preview Card
                let prev_card = RECT {
                    left: 250,
                    top: 365,
                    right: width - 30,
                    bottom: height - 60,
                };
                draw_rounded_card(mem_dc, &prev_card, rgb(24, 24, 30), rgb(39, 39, 48));
                unsafe {
                    let _ = SelectObject(mem_dc, hfont_section.into());
                    SetTextColor(mem_dc, theme.primary_light);
                }
                draw_text_str(
                    mem_dc,
                    "👁️  CANLI GÖRÜNÜM ÖNİZLEMESİ",
                    270,
                    380,
                    width - 50,
                    405,
                    DT_LEFT | DT_SINGLELINE,
                );

                unsafe {
                    let _ = SelectObject(mem_dc, hfont_body.into());
                    SetTextColor(mem_dc, rgb(212, 212, 216));
                }
                let prev_info = format!(
                    "Seçili Tema: {} | Saydamlık: %{}\nArama penceresi açıldığında arayüz renkleri ve pencere saydamlığı anında bu ayarlarla güncellenir.",
                    theme.name, state.config.opacity_percent
                );
                draw_text_str(
                    mem_dc,
                    &prev_info,
                    270,
                    415,
                    width - 50,
                    height - 70,
                    DT_LEFT | DT_WORDBREAK,
                );
            }
            SettingsTab::Guide => {
                unsafe {
                    let _ = SelectObject(mem_dc, hfont_title.into());
                    SetTextColor(mem_dc, rgb(255, 255, 255));
                }
                draw_text_str(
                    mem_dc,
                    "Kılavuz, Komutlar & İpuçları",
                    content_x,
                    22,
                    width - 100,
                    48,
                    DT_LEFT | DT_SINGLELINE,
                );

                unsafe {
                    let _ = SelectObject(mem_dc, hfont_body.into());
                    SetTextColor(mem_dc, rgb(161, 161, 170));
                }
                draw_text_str(
                    mem_dc,
                    "LiteCast'ten en yüksek verimi almak için pratik kullanım rehberi.",
                    content_x,
                    50,
                    width - 100,
                    72,
                    DT_LEFT | DT_SINGLELINE,
                );

                let guide_card = RECT {
                    left: 250,
                    top: 90,
                    right: width - 30,
                    bottom: height - 60,
                };
                draw_rounded_card(mem_dc, &guide_card, rgb(24, 24, 30), rgb(39, 39, 48));

                unsafe {
                    let _ = SelectObject(mem_dc, hfont_section.into());
                    SetTextColor(mem_dc, theme.primary_light);
                }
                draw_text_str(
                    mem_dc,
                    "🚀  LITECAST 1.0 SÜPER GÜÇLERİ & KULLANIM REHBERİ",
                    270,
                    105,
                    width - 50,
                    130,
                    DT_LEFT | DT_SINGLELINE,
                );

                unsafe {
                    let _ = SelectObject(mem_dc, hfont_body.into());
                    SetTextColor(mem_dc, rgb(228, 228, 231));
                }
                let guide_text = "✨ 8 SÜPER GÜÇ VE KLAVYE KISAYOLLARI:\n\n1. 📁 Dosya Gezgini: 'C:\\', 'D:\\', '%appdata%', '~/' yazarak dosya ve klasörleri listeleyin, boyuta göre inceleyin.\n2. 💻 Terminal Runner: '> ping google.com', '$ git status', 'cmd:' veya 'ps:' ile komut koşturun.\n3. ⚡ Hızlı Eylemler (Action Menu): Herhangi bir sonuçta 'Tab' veya 'Ctrl+K' basarak eylem menüsünü açın.\n4. 🛡️ Doğrudan Yönetici: Arama sonucunda 'Ctrl+Shift+Enter' basarak yönetici yetkileriyle başlatın.\n5. 📂 Dosya Konumu: 'Ctrl+E' basarak seçili uygulamanın/dosyanın klasörünü Windows Explorer'da seçili açın.\n6. 📋 Pano & Değer Kopyala: 'Ctrl+C' ile dosya yolunu kopyalayın; 'cb' yazarak son 50 pano geçmişini yönetin.\n7. 🧮 Hesaplama & Dönüştürücüler: '100 km to mi', '100 c to f', '5 kg to lbs', '100 usd in try', 'unix 1700000000'.\n8. 🛠️ Geliştirici Araçları: 'json {...}' (Format/Minify), 'jwt ey...' (Token çöz), 'lorem 3p', 'cron */15 * * * *'.";
                draw_text_str(
                    mem_dc,
                    guide_text,
                    270,
                    138,
                    width - 50,
                    height - 70,
                    DT_LEFT | DT_WORDBREAK,
                );
            }
        }

        // 6. Footer: Status Message
        let footer_rect = RECT {
            left: 245,
            top: height - 48,
            right: width - 16,
            bottom: height - 16,
        };
        draw_rounded_card(mem_dc, &footer_rect, rgb(22, 22, 26), rgb(36, 36, 42));
        unsafe {
            let _ = SelectObject(mem_dc, hfont_small.into());
            SetTextColor(mem_dc, rgb(161, 161, 170));
        }
        let footer_msg = format!(
            "Bilgi: {}  •  Pencereyi kapatsanız bile kısayol arka planda çalışır.",
            state.last_action_msg
        );
        draw_text_str(
            mem_dc,
            &footer_msg,
            260,
            height - 48,
            width - 30,
            height - 16,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );

        // Delete font objects
        unsafe {
            let _ = DeleteObject(hfont_brand.into());
            let _ = DeleteObject(hfont_small.into());
            let _ = DeleteObject(hfont_tab.into());
            let _ = DeleteObject(hfont_title.into());
            let _ = DeleteObject(hfont_section.into());
            let _ = DeleteObject(hfont_body.into());
            let _ = DeleteObject(hfont_bold.into());
            let _ = DeleteObject(hfont_ctrl.into());
        }
    }

    // Now bitblt to window DC with memory bitmap intact, and then cleanup
    unsafe {
        let _ = BitBlt(hdc, 0, 0, width, height, Some(mem_dc), 0, 0, SRCCOPY);
        let _ = SelectObject(mem_dc, old_bmp);
        let _ = DeleteObject(mem_bitmap.into());
        let _ = DeleteDC(mem_dc);
        let _ = EndPaint(hwnd, &ps);
    }
}

fn draw_rounded_card(hdc: HDC, rect: &RECT, bg_color: COLORREF, border_color: COLORREF) {
    unsafe {
        let border_brush = CreateSolidBrush(border_color);
        FillRect(hdc, rect, border_brush);
        let _ = DeleteObject(border_brush.into());

        let inner_rect = RECT {
            left: rect.left + 1,
            top: rect.top + 1,
            right: rect.right - 1,
            bottom: rect.bottom - 1,
        };
        let bg_brush = CreateSolidBrush(bg_color);
        FillRect(hdc, &inner_rect, bg_brush);
        let _ = DeleteObject(bg_brush.into());
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
            windows::Win32::Graphics::Gdi::FONT_CHARSET(1),
            FONT_OUTPUT_PRECISION(0),
            FONT_CLIP_PRECISION(0),
            FONT_QUALITY(5),
            0,
            PCWSTR::from_raw(font_name.as_ptr()),
        )
    }
}

fn draw_text_str(
    hdc: HDC,
    text: &str,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    flags: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    let mut wide = to_wide(text);
    let mut rect = RECT {
        left,
        top,
        right,
        bottom,
    };
    unsafe {
        let len = wide.len() - 1;
        let _ = DrawTextW(hdc, &mut wide[..len], &mut rect, flags | DT_NOPREFIX);
    }
}
