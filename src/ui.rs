use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreateSolidBrush,
    DT_CENTER, DT_LEFT, DT_NOPREFIX, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, EndPaint,
    FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FONT_QUALITY, FillRect, HDC, HFONT, InvalidateRect,
    PAINTSTRUCT, SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DI_NORMAL, DefWindowProcW, DispatchMessageW,
    DrawIconEx, GetMessageW, IDC_ARROW, IDC_HAND, LoadCursorW, MSG, PostQuitMessage,
    RegisterClassExW, SW_HIDE, SW_RESTORE, SW_SHOW, SetCursor, SetForegroundWindow, SetWindowPos,
    ShowWindow, TranslateMessage, WM_CHAR, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_HOTKEY,
    WM_KEYDOWN, WM_KILLFOCUS, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_PAINT, WM_RBUTTONUP, WM_SYSCOMMAND, WNDCLASSEXW, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::clipboard::{check_and_record_current_clipboard, init_clipboard_manager};
use crate::commands::help::{get_help_content, get_hotkeys_content, get_install_help_content};
use crate::commands::{CommandAction, parse_command};
use crate::config::{AppConfig, rgb};
use crate::history::UsageHistory;
use crate::indexer::AppIndex;
use crate::platform_windows::{
    HOTKEY_ID, IDM_OPEN_LAUNCHER, IDM_OPEN_SETTINGS, IDM_QUIT, IDM_REINDEX, WM_TOGGLE_LAUNCHER,
    WM_TRAYICON, add_tray_icon, apply_modern_window_styling, center_window_on_active_monitor,
    enable_dpi_awareness, get_app_icon, init_keyboard_hook, launch_target, launch_target_elevated,
    register_configured_hotkey, remove_tray_icon, set_window_icons, set_window_opacity,
    show_tray_menu, to_wide, unhook_keyboard_hook, unregister_hotkey,
};
use crate::search::{ResultAction, SearchResult, search_apps_with_history};
use crate::settings_ui::{
    LAUNCHER_HWND, SETTINGS_HEIGHT, SETTINGS_HWND, SETTINGS_WIDTH, init_settings_state,
    register_settings_class, show_settings_window,
};
use crate::telemetry_local::{log_error, log_info};

pub const WINDOW_WIDTH: i32 = 780;
pub const BASE_WINDOW_HEIGHT: i32 = 74;
pub const RESULT_ROW_HEIGHT: i32 = 54;
pub const VISIBLE_ROWS: usize = 7;
pub const MAX_SEARCH_RESULTS: usize = 100;

#[derive(Debug, Clone)]
pub enum ViewMode {
    Search,
    Help(String),
}

#[derive(Debug, Clone)]
pub struct ContextActionItem {
    pub key_hint: &'static str,
    pub title: String,
    pub subtitle: String,
    pub badge: &'static str,
    pub action: ResultAction,
}

pub fn get_context_actions_for_result(result: &SearchResult) -> Vec<ContextActionItem> {
    let mut actions = Vec::new();

    // 1. Default Run / Open
    actions.push(ContextActionItem {
        key_hint: "Enter",
        title: format!("⚡ {}", result.title),
        subtitle: format!("Varsayılan eylem: {}", result.subtitle),
        badge: "Varsayılan",
        action: result.action.clone(),
    });

    // Extract target path if applicable
    let target_path = match &result.action {
        ResultAction::LaunchApp { target, .. } => Some(target.clone()),
        ResultAction::LaunchElevated { target, .. } => Some(target.clone()),
        _ => None,
    };

    // 2. Run as Administrator
    if let Some(target) = &target_path {
        actions.push(ContextActionItem {
            key_hint: "Ctrl+Shift+Enter",
            title: "🛡️ Yönetici Olarak Çalıştır".to_string(),
            subtitle: format!("{} (Yönetici İzinleri)", target),
            badge: "Yönetici",
            action: ResultAction::LaunchElevated {
                target: target.clone(),
                args: None,
                working_dir: None,
            },
        });

        // 3. Show in File Explorer
        actions.push(ContextActionItem {
            key_hint: "Ctrl+E",
            title: "📁 Dosya Konumunu Aç".to_string(),
            subtitle: format!("Explorer'da göster: {}", target),
            badge: "Gezgin",
            action: ResultAction::LaunchApp {
                target: "explorer.exe".to_string(),
                args: Some(format!("/select,\"{}\"", target)),
                item_id: format!("open_dir_{}", target),
            },
        });

        // 4. Copy Path
        actions.push(ContextActionItem {
            key_hint: "Ctrl+C",
            title: "📋 Dosya Yolunu Kopyala".to_string(),
            subtitle: target.clone(),
            badge: "Kopyala",
            action: ResultAction::CopyToClipboard {
                text: target.clone(),
                notify_msg: format!("Dosya yolu kopyalandı: {}", target),
            },
        });
    } else if let ResultAction::CopyToClipboard { text, .. } = &result.action {
        actions.push(ContextActionItem {
            key_hint: "Ctrl+C",
            title: "📋 Değeri Panoya Kopyala".to_string(),
            subtitle: text.clone(),
            badge: "Kopyala",
            action: result.action.clone(),
        });
    } else if let ResultAction::OpenUrl { url } = &result.action {
        actions.push(ContextActionItem {
            key_hint: "Ctrl+C",
            title: "📋 URL Adresini Kopyala".to_string(),
            subtitle: url.clone(),
            badge: "Kopyala",
            action: ResultAction::CopyToClipboard {
                text: url.clone(),
                notify_msg: format!("URL kopyalandı: {}", url),
            },
        });
    }

    // 5. Search on Google
    actions.push(ContextActionItem {
        key_hint: "Alt+G",
        title: format!("🌐 Google'da Ara: \"{}\"", result.title),
        subtitle: format!(
            "https://www.google.com/search?q={}",
            crate::web_search::url_encode(&result.title)
        ),
        badge: "Web Arama",
        action: ResultAction::OpenUrl {
            url: format!(
                "https://www.google.com/search?q={}",
                crate::web_search::url_encode(&result.title)
            ),
        },
    });

    actions
}

pub struct UiState {
    pub query: String,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub results: Vec<SearchResult>,
    pub view_mode: ViewMode,
    pub help_scroll_offset: i32,
    pub hovered_settings_btn: bool,
    pub is_action_menu_open: bool,
    pub action_menu_index: usize,
    pub index: AppIndex,
    pub history: UsageHistory,
}

pub static UI_STATE: Mutex<Option<UiState>> = Mutex::new(None);
pub static SHOULD_QUIT: AtomicBool = AtomicBool::new(false);

pub fn update_launcher_index(new_index: AppIndex) {
    let config = AppConfig::load();
    let mut state_guard = UI_STATE.lock().unwrap();
    if let Some(state) = state_guard.as_mut() {
        state.index = new_index;
        state.results = search_apps_with_history(
            &state.index.items,
            &state.query,
            &state.history,
            &config,
            MAX_SEARCH_RESULTS,
        );
        state.selected_index = 0;
        state.scroll_offset = 0;
    }
}

pub fn run_ui() {
    unsafe {
        // Enable crisp Per-Monitor DPI Awareness
        enable_dpi_awareness();

        // Initialize Clipboard Manager
        init_clipboard_manager();

        let hinstance = HINSTANCE(GetModuleHandleW(PCWSTR::null()).unwrap().0);
        let class_name = w!("LiteCastWindowClass");

        let icon = get_app_icon(64);
        let icon_sm = get_app_icon(32);

        let wnd_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance,
            hIcon: icon,
            hIconSm: icon_sm,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: class_name,
            ..Default::default()
        };

        if RegisterClassExW(&wnd_class) == 0 {
            log_error("Failed to register launcher window class.");
            return;
        }

        // Register Settings Window class
        if !register_settings_class(hinstance) {
            log_error("Failed to register settings window class.");
        }

        // 1. Create Launcher Window (Frameless popup)
        let hwnd_launcher = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            w!("LiteCast"),
            WS_POPUP,
            0,
            0,
            WINDOW_WIDTH,
            BASE_WINDOW_HEIGHT,
            Some(HWND::default()),
            None,
            Some(hinstance),
            None,
        )
        .unwrap();

        if hwnd_launcher.0.is_null() {
            log_error("Failed to create launcher window.");
            return;
        }

        if !hwnd_launcher.0.is_null() {
            apply_modern_window_styling(hwnd_launcher);
        }

        // Store launcher handle
        {
            let mut guard = LAUNCHER_HWND.lock().unwrap();
            *guard = Some(hwnd_launcher.0 as isize);
        }

        // Apply icons and opacity
        set_window_icons(hwnd_launcher);
        let config = AppConfig::load();
        set_window_opacity(hwnd_launcher, config.opacity_percent);

        // 2. Create Settings / Dashboard Window
        let settings_title = to_wide("LiteCast - Kontrol Paneli & Ayarlar");
        let hwnd_settings = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            PCWSTR::from_raw(to_wide("LiteCastSettingsClass").as_ptr()),
            PCWSTR::from_raw(settings_title.as_ptr()),
            WS_POPUP,
            100,
            100,
            SETTINGS_WIDTH,
            SETTINGS_HEIGHT,
            Some(HWND::default()),
            None,
            Some(hinstance),
            None,
        )
        .unwrap_or_default();

        if !hwnd_settings.0.is_null() {
            apply_modern_window_styling(hwnd_settings);
            set_window_icons(hwnd_settings);
            center_window_on_active_monitor(hwnd_settings, SETTINGS_WIDTH, SETTINGS_HEIGHT);
        }

        // Initialize Low-Level Keyboard Hook for 100% reliable hotkey capture (Alt+Space etc.)
        init_keyboard_hook(hwnd_launcher);

        // Also Register standard hotkey
        if !register_configured_hotkey(hwnd_launcher, config.hotkey_index) {
            log_info("Standard RegisterHotKey active alongside low-level keyboard hook.");
        }

        // Load Index & History
        let index = AppIndex::load_or_rebuild();
        let index_count = index.items.len();
        let history = UsageHistory::load();
        let initial_results =
            search_apps_with_history(&index.items, "", &history, &config, MAX_SEARCH_RESULTS);

        {
            let mut state = UI_STATE.lock().unwrap();
            *state = Some(UiState {
                query: String::new(),
                selected_index: 0,
                scroll_offset: 0,
                results: initial_results,
                view_mode: ViewMode::Search,
                help_scroll_offset: 0,
                hovered_settings_btn: false,
                is_action_menu_open: false,
                action_menu_index: 0,
                index,
                history,
            });
        }

        // Init Settings UI state
        init_settings_state(index_count);

        // Add System Tray Icon
        if add_tray_icon(hwnd_launcher) {
            log_info("Added LiteCast icon to Windows System Tray.");
        }

        // Only show Settings Window if explicitly requested via command line (e.g. --settings)
        let show_settings_on_start =
            std::env::args().any(|arg| arg == "--settings" || arg == "-s" || arg == "--config");

        if show_settings_on_start && !hwnd_settings.0.is_null() {
            show_settings_window(hwnd_settings);
        }

        // Message Loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if SHOULD_QUIT.load(Ordering::Relaxed) {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Cleanup
        unhook_keyboard_hook();
        remove_tray_icon(hwnd_launcher);
        unregister_hotkey(hwnd_launcher);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            log_info(&format!("WM_HOTKEY received: {}", wparam.0));
            if wparam.0 == HOTKEY_ID as usize {
                toggle_window(hwnd);
            }
            LRESULT(0)
        }
        WM_TOGGLE_LAUNCHER => {
            toggle_window(hwnd);
            LRESULT(0)
        }
        WM_SYSCOMMAND => {
            // Suppress SC_KEYMENU (0xF100) so Alt+Space never pops up window title bar menu
            if (wparam.0 & 0xFFF0) == 0xF100 {
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_TRAYICON => {
            let event = (lparam.0 & 0xffff) as u32;
            match event {
                WM_RBUTTONUP => {
                    let cmd = show_tray_menu(hwnd);
                    match cmd {
                        IDM_OPEN_LAUNCHER => {
                            show_window(hwnd);
                        }
                        IDM_OPEN_SETTINGS => {
                            let settings_val = *SETTINGS_HWND.lock().unwrap();
                            if let Some(h) = settings_val {
                                show_settings_window(HWND(h as *mut _));
                            }
                        }
                        IDM_REINDEX => {
                            let new_index = AppIndex::rebuild();
                            update_launcher_index(new_index);
                        }
                        IDM_QUIT => {
                            unsafe {
                                let _ = ShowWindow(hwnd, SW_HIDE);
                            }
                            SHOULD_QUIT.store(true, Ordering::Relaxed);
                            unsafe {
                                PostQuitMessage(0);
                            }
                        }
                        _ => {}
                    }
                }
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
                    toggle_window(hwnd);
                }
                _ => {}
            }
            LRESULT(0)
        }
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
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xffff) as i16;
            handle_mouse_wheel(hwnd, delta);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wparam.0 as u32;
            handle_keydown(hwnd, vk);
            LRESULT(0)
        }
        WM_CHAR => {
            let ch = char::from_u32(wparam.0 as u32).unwrap_or('\0');
            handle_char(hwnd, ch);
            LRESULT(0)
        }
        WM_PAINT => {
            unsafe {
                paint_window(hwnd);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_KILLFOCUS => {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub fn toggle_window(hwnd: HWND) {
    unsafe {
        let is_visible = windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool();
        if is_visible {
            let _ = ShowWindow(hwnd, SW_HIDE);
        } else {
            show_window(hwnd);
        }
    }
}

pub fn show_window(hwnd: HWND) {
    unsafe {
        log_info("show_window activated");
        check_and_record_current_clipboard();

        let config = AppConfig::load();
        set_window_opacity(hwnd, config.opacity_percent);

        {
            let mut state_guard = UI_STATE.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                state.query.clear();
                state.selected_index = 0;
                state.scroll_offset = 0;
                state.hovered_settings_btn = false;
                state.is_action_menu_open = false;
                state.action_menu_index = 0;
                state.view_mode = ViewMode::Search;
                state.results = search_apps_with_history(
                    &state.index.items,
                    "",
                    &state.history,
                    &config,
                    MAX_SEARCH_RESULTS,
                );
            }
        }

        let height = calculate_current_height();
        center_window_on_active_monitor(hwnd, WINDOW_WIDTH, height);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = windows::Win32::UI::WindowsAndMessaging::BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetActiveWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

fn calculate_current_height() -> i32 {
    let state_guard = UI_STATE.lock().unwrap();
    if let Some(state) = state_guard.as_ref() {
        if state.is_action_menu_open
            && let Some(sel) = state.results.get(state.selected_index)
        {
            let actions = get_context_actions_for_result(sel);
            return BASE_WINDOW_HEIGHT + (actions.len().min(6) as i32 * RESULT_ROW_HEIGHT) + 16;
        }

        match &state.view_mode {
            ViewMode::Search => {
                if state.results.is_empty() {
                    BASE_WINDOW_HEIGHT
                } else {
                    let visible_count = state.results.len().min(VISIBLE_ROWS);
                    BASE_WINDOW_HEIGHT + (visible_count as i32 * RESULT_ROW_HEIGHT) + 10
                }
            }
            ViewMode::Help(_) => 520,
        }
    } else {
        BASE_WINDOW_HEIGHT
    }
}

fn handle_mouse_move(hwnd: HWND, x: i32, y: i32) {
    let is_visible =
        unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() };
    if !is_visible {
        return;
    }

    let mut need_repaint = false;
    let mut hand_cursor = false;

    {
        let mut state_guard = UI_STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            // Check settings gear button hover (x: WINDOW_WIDTH - 64 .. WINDOW_WIDTH - 16, y: 16 .. 58)
            let is_settings_hover =
                ((WINDOW_WIDTH - 64)..=(WINDOW_WIDTH - 16)).contains(&x) && (16..=58).contains(&y);
            if state.hovered_settings_btn != is_settings_hover {
                state.hovered_settings_btn = is_settings_hover;
                need_repaint = true;
            }
            if is_settings_hover {
                hand_cursor = true;
            }

            // Check item hover if in search view and y >= 78
            if let ViewMode::Search = state.view_mode
                && y >= 78
                && (14..=(WINDOW_WIDTH - 14)).contains(&x)
            {
                let visible_row = ((y - 78) / RESULT_ROW_HEIGHT) as usize;
                let target_index = state.scroll_offset + visible_row;
                if target_index < state.results.len() {
                    if state.selected_index != target_index {
                        state.selected_index = target_index;
                        need_repaint = true;
                    }
                    hand_cursor = true;
                }
            }
        }
    }

    unsafe {
        if hand_cursor && let Ok(hand) = LoadCursorW(None, IDC_HAND) {
            let _ = SetCursor(Some(hand));
        }
        if need_repaint {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }
}

fn execute_result_action(action: &ResultAction, history: &mut UsageHistory) {
    match action {
        ResultAction::LaunchApp {
            target,
            args,
            item_id,
        } => {
            log_info(&format!("Launching application: {}", target));
            *history.launches.entry(item_id.clone()).or_insert(0) += 1;
            let hist_clone = history.clone();
            let target_clone = target.clone();
            let args_clone = args.clone();
            std::thread::spawn(move || {
                hist_clone.save();
                if let Err(e) = launch_target(&target_clone, args_clone.as_deref()) {
                    log_error(&format!("Launch failed: {}", e));
                }
            });
        }
        ResultAction::LaunchElevated {
            target,
            args,
            working_dir,
        } => {
            log_info(&format!("Launching elevated application: {}", target));
            let target_clone = target.clone();
            let args_clone = args.clone();
            let working_dir_clone = working_dir.clone();
            std::thread::spawn(move || {
                if let Err(e) = launch_target_elevated(
                    &target_clone,
                    args_clone.as_deref(),
                    working_dir_clone.as_deref(),
                ) {
                    log_error(&format!("Elevated launch failed: {}", e));
                }
            });
        }
        ResultAction::CopyToClipboard { text, notify_msg } => {
            log_info(&format!("Copying to clipboard: {}", notify_msg));
            if let Err(e) = crate::clipboard::set_clipboard_text(text) {
                log_error(&format!("Clipboard copy failed: {}", e));
            }
        }
        ResultAction::OpenUrl { url } => {
            log_info(&format!("Opening URL in browser: {}", url));
            let url_clone = url.clone();
            std::thread::spawn(move || {
                if let Err(e) = launch_target(&url_clone, None) {
                    log_error(&format!("Failed to open URL: {}", e));
                }
            });
        }
        ResultAction::ExecuteSystem { kind } => {
            log_info(&format!("Executing system action: {:?}", kind));
            let kind_clone = kind.clone();
            std::thread::spawn(move || {
                match crate::system_actions::execute_system_action(&kind_clone) {
                    Ok(msg) => log_info(&format!("System action success: {}", msg)),
                    Err(e) => log_error(&format!("System action error: {}", e)),
                }
            });
        }
        ResultAction::FocusWindow { hwnd_val } => {
            log_info(&format!("Focusing window: {}", hwnd_val));
            crate::window_switcher::focus_window(*hwnd_val);
        }
    }
}

fn handle_mouse_click(hwnd: HWND, x: i32, y: i32) {
    let is_visible =
        unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() };
    if !is_visible {
        return;
    }

    let mut action_to_execute: Option<ResultAction> = None;
    let mut open_settings = false;
    let mut should_hide = false;

    {
        let mut state_guard = UI_STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            // Clicked settings gear button
            if ((WINDOW_WIDTH - 64)..=(WINDOW_WIDTH - 16)).contains(&x) && (16..=58).contains(&y) {
                open_settings = true;
                should_hide = true;
            } else if let ViewMode::Search = state.view_mode
                && y >= 78
                && (14..=(WINDOW_WIDTH - 14)).contains(&x)
            {
                let visible_row = ((y - 78) / RESULT_ROW_HEIGHT) as usize;
                let target_index = state.scroll_offset + visible_row;
                if let Some(sel) = state.results.get(target_index) {
                    action_to_execute = Some(sel.action.clone());
                    should_hide = true;
                }
            }
        }
    }

    // Hide immediately before action execution so popup vanishes with 0ms latency
    if should_hide {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }

    if open_settings {
        let settings_val = *SETTINGS_HWND.lock().unwrap();
        if let Some(h) = settings_val {
            show_settings_window(HWND(h as *mut _));
        }
    }

    if let Some(act) = action_to_execute {
        let mut state_guard = UI_STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            execute_result_action(&act, &mut state.history);
        }
    }
}

fn handle_mouse_wheel(hwnd: HWND, delta: i16) {
    let is_visible =
        unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() };
    if !is_visible {
        return;
    }

    let mut need_repaint = false;

    {
        let mut state_guard = UI_STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            match &mut state.view_mode {
                ViewMode::Search => {
                    let max_scroll = state.results.len().saturating_sub(VISIBLE_ROWS);
                    if delta > 0 {
                        // Scroll Up
                        if state.scroll_offset > 0 {
                            state.scroll_offset = state.scroll_offset.saturating_sub(1);
                            need_repaint = true;
                        }
                    } else if delta < 0 {
                        // Scroll Down
                        if state.scroll_offset < max_scroll {
                            state.scroll_offset = (state.scroll_offset + 1).min(max_scroll);
                            need_repaint = true;
                        }
                    }
                }
                ViewMode::Help(_) => {
                    if delta > 0 {
                        state.help_scroll_offset = (state.help_scroll_offset - 60).max(0);
                        need_repaint = true;
                    } else if delta < 0 {
                        state.help_scroll_offset += 60;
                        need_repaint = true;
                    }
                }
            }
        }
    }

    if need_repaint {
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }
}

fn handle_keydown(hwnd: HWND, vk: u32) {
    let is_visible =
        unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() };
    if !is_visible {
        return;
    }

    let mut need_repaint = false;
    let mut need_resize = false;
    let mut action_to_execute: Option<ResultAction> = None;
    let mut should_hide = false;
    let mut should_quit = false;

    let config = AppConfig::load();

    let is_ctrl = unsafe {
        (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(
            windows::Win32::UI::Input::KeyboardAndMouse::VK_CONTROL.0 as i32,
        ) as u16
            & 0x8000)
            != 0
    };
    let is_shift = unsafe {
        (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(
            windows::Win32::UI::Input::KeyboardAndMouse::VK_SHIFT.0 as i32,
        ) as u16
            & 0x8000)
            != 0
    };

    {
        let mut state_guard = UI_STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            // Global Shortcut 1: Ctrl + Shift + Enter -> Run as Administrator immediately
            if is_ctrl && is_shift && vk == 0x0D {
                if let Some(sel) = state.results.get(state.selected_index) {
                    let actions = get_context_actions_for_result(sel);
                    if let Some(admin_act) = actions.iter().find(|a| a.badge == "Yönetici") {
                        action_to_execute = Some(admin_act.action.clone());
                        should_hide = true;
                    } else {
                        action_to_execute = Some(sel.action.clone());
                        should_hide = true;
                    }
                }
            }
            // Global Shortcut 2: Ctrl + E -> Show in File Explorer immediately
            else if is_ctrl && vk == 0x45 {
                if let Some(sel) = state.results.get(state.selected_index) {
                    let actions = get_context_actions_for_result(sel);
                    if let Some(exp_act) = actions.iter().find(|a| a.badge == "Gezgin") {
                        action_to_execute = Some(exp_act.action.clone());
                        should_hide = true;
                    }
                }
            }
            // Global Shortcut 3: Ctrl + Shift + C -> Copy Path immediately
            else if is_ctrl && is_shift && vk == 0x43 {
                if let Some(sel) = state.results.get(state.selected_index) {
                    let actions = get_context_actions_for_result(sel);
                    if let Some(copy_act) = actions.iter().find(|a| a.badge == "Kopyala") {
                        action_to_execute = Some(copy_act.action.clone());
                        should_hide = true;
                    }
                }
            }
            // Global Shortcut 4: Tab (0x09) or Ctrl + K -> Toggle Action Menu
            else if vk == 0x09 || (is_ctrl && vk == 0x4B) {
                if !state.results.is_empty() {
                    state.is_action_menu_open = !state.is_action_menu_open;
                    state.action_menu_index = 0;
                    need_resize = true;
                    need_repaint = true;
                }
            }
            // When Action Menu is OPEN
            else if state.is_action_menu_open {
                let actions = if let Some(sel) = state.results.get(state.selected_index) {
                    get_context_actions_for_result(sel)
                } else {
                    Vec::new()
                };

                match vk {
                    0x1B => {
                        // Escape: Close action menu back to search view
                        state.is_action_menu_open = false;
                        need_resize = true;
                        need_repaint = true;
                    }
                    0x26 => {
                        // Arrow Up
                        if state.action_menu_index > 0 {
                            state.action_menu_index -= 1;
                            need_repaint = true;
                        }
                    }
                    0x28 => {
                        // Arrow Down
                        if !actions.is_empty() && state.action_menu_index + 1 < actions.len() {
                            state.action_menu_index += 1;
                            need_repaint = true;
                        }
                    }
                    0x0D => {
                        // Enter: Execute selected action
                        if let Some(act_item) = actions.get(state.action_menu_index) {
                            action_to_execute = Some(act_item.action.clone());
                            should_hide = true;
                        }
                    }
                    0x31..=0x39 => {
                        // Numeric shortcuts (1..9)
                        let idx = (vk - 0x31) as usize;
                        if let Some(act_item) = actions.get(idx) {
                            action_to_execute = Some(act_item.action.clone());
                            should_hide = true;
                        }
                    }
                    _ => {}
                }
            }
            // Normal Search View Handling
            else {
                match vk {
                    0x1B => match state.view_mode {
                        ViewMode::Search => {
                            should_hide = true;
                        }
                        ViewMode::Help(_) => {
                            state.view_mode = ViewMode::Search;
                            state.help_scroll_offset = 0;
                            need_resize = true;
                            need_repaint = true;
                        }
                    },
                    0x26 => match state.view_mode {
                        ViewMode::Search => {
                            if state.selected_index > 0 {
                                state.selected_index -= 1;
                                if state.selected_index < state.scroll_offset {
                                    state.scroll_offset = state.selected_index;
                                }
                                need_repaint = true;
                            }
                        }
                        ViewMode::Help(_) => {
                            state.help_scroll_offset = (state.help_scroll_offset - 40).max(0);
                            need_repaint = true;
                        }
                    },
                    0x28 => match state.view_mode {
                        ViewMode::Search => {
                            if !state.results.is_empty()
                                && state.selected_index + 1 < state.results.len()
                            {
                                state.selected_index += 1;
                                if state.selected_index >= state.scroll_offset + VISIBLE_ROWS {
                                    state.scroll_offset = state.selected_index - VISIBLE_ROWS + 1;
                                }
                                need_repaint = true;
                            }
                        }
                        ViewMode::Help(_) => {
                            state.help_scroll_offset += 40;
                            need_repaint = true;
                        }
                    },
                    0x21 => match state.view_mode {
                        ViewMode::Search => {
                            if state.selected_index > 0 {
                                state.selected_index =
                                    state.selected_index.saturating_sub(VISIBLE_ROWS);
                                if state.selected_index < state.scroll_offset {
                                    state.scroll_offset = state.selected_index;
                                }
                                need_repaint = true;
                            }
                        }
                        ViewMode::Help(_) => {
                            state.help_scroll_offset = (state.help_scroll_offset - 200).max(0);
                            need_repaint = true;
                        }
                    },
                    0x22 => match state.view_mode {
                        ViewMode::Search => {
                            if !state.results.is_empty() {
                                state.selected_index = (state.selected_index + VISIBLE_ROWS)
                                    .min(state.results.len() - 1);
                                if state.selected_index >= state.scroll_offset + VISIBLE_ROWS {
                                    state.scroll_offset = state.selected_index - VISIBLE_ROWS + 1;
                                }
                                need_repaint = true;
                            }
                        }
                        ViewMode::Help(_) => {
                            state.help_scroll_offset += 200;
                            need_repaint = true;
                        }
                    },
                    0x0D => {
                        if let Some(action) = parse_command(&state.query) {
                            match action {
                                CommandAction::OpenSettings => {
                                    let settings_val = *SETTINGS_HWND.lock().unwrap();
                                    if let Some(h) = settings_val {
                                        show_settings_window(HWND(h as *mut _));
                                    }
                                    should_hide = true;
                                }
                                CommandAction::OpenHelp => {
                                    state.view_mode =
                                        ViewMode::Help(get_help_content().to_string());
                                    state.help_scroll_offset = 0;
                                    need_resize = true;
                                    need_repaint = true;
                                }
                                CommandAction::OpenInstallHelp => {
                                    state.view_mode =
                                        ViewMode::Help(get_install_help_content().to_string());
                                    state.help_scroll_offset = 0;
                                    need_resize = true;
                                    need_repaint = true;
                                }
                                CommandAction::OpenHotkeysHelp => {
                                    state.view_mode =
                                        ViewMode::Help(get_hotkeys_content().to_string());
                                    state.help_scroll_offset = 0;
                                    need_resize = true;
                                    need_repaint = true;
                                }
                                CommandAction::ToggleAutostart
                                | CommandAction::EnableAutostart
                                | CommandAction::DisableAutostart => {
                                    let settings_val = *SETTINGS_HWND.lock().unwrap();
                                    if let Some(h) = settings_val {
                                        show_settings_window(HWND(h as *mut _));
                                    }
                                    should_hide = true;
                                }
                                CommandAction::Reindex => {
                                    state.index = AppIndex::rebuild();
                                    state.results = search_apps_with_history(
                                        &state.index.items,
                                        &state.query,
                                        &state.history,
                                        &config,
                                        MAX_SEARCH_RESULTS,
                                    );
                                    state.selected_index = 0;
                                    state.scroll_offset = 0;
                                    need_resize = true;
                                    need_repaint = true;
                                }
                                CommandAction::Quit => {
                                    should_quit = true;
                                }
                            }
                        } else if let ViewMode::Search = state.view_mode
                            && let Some(sel) = state.results.get(state.selected_index)
                        {
                            action_to_execute = Some(sel.action.clone());
                            should_hide = true;
                        }
                    }
                    0x08 => {
                        if let ViewMode::Search = state.view_mode
                            && !state.query.is_empty()
                        {
                            state.query.pop();
                            state.results = search_apps_with_history(
                                &state.index.items,
                                &state.query,
                                &state.history,
                                &config,
                                MAX_SEARCH_RESULTS,
                            );
                            state.selected_index = 0;
                            state.scroll_offset = 0;
                            need_resize = true;
                            need_repaint = true;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Hide immediately before action execution so popup vanishes with 0ms latency
    if should_hide {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }

    if let Some(act) = action_to_execute {
        let mut state_guard = UI_STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            execute_result_action(&act, &mut state.history);
        }
    }

    if should_quit {
        SHOULD_QUIT.store(true, Ordering::Relaxed);
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
            PostQuitMessage(0);
        }
    }

    if need_resize {
        let height = calculate_current_height();
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                Some(HWND::default()),
                0,
                0,
                WINDOW_WIDTH,
                height,
                windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                    | windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER
                    | windows::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE,
            );
        }
    }

    if need_repaint {
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }
}

fn handle_char(hwnd: HWND, ch: char) {
    let is_visible =
        unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() };
    if !is_visible {
        return;
    }

    if ch.is_control() {
        return;
    }

    let config = AppConfig::load();
    let mut need_repaint = false;
    let mut need_resize = false;

    {
        let mut state_guard = UI_STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut()
            && let ViewMode::Search = state.view_mode
        {
            state.query.push(ch);
            state.results = search_apps_with_history(
                &state.index.items,
                &state.query,
                &state.history,
                &config,
                MAX_SEARCH_RESULTS,
            );
            state.selected_index = 0;
            state.scroll_offset = 0;
            need_resize = true;
            need_repaint = true;
        }
    }

    if need_resize {
        let height = calculate_current_height();
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                Some(HWND::default()),
                0,
                0,
                WINDOW_WIDTH,
                height,
                windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                    | windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER
                    | windows::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE,
            );
        }
    }

    if need_repaint {
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }
}

unsafe fn paint_window(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };

    let mut rect = RECT::default();
    let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect) };
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    let mem_dc = unsafe { CreateCompatibleDC(Some(hdc)) };
    let mem_bitmap = unsafe { CreateCompatibleBitmap(hdc, width, height) };
    let old_bmp = unsafe { SelectObject(mem_dc, mem_bitmap.into()) };

    // Obsidian Background
    let bg_color = rgb(18, 18, 22);
    let brush = unsafe { CreateSolidBrush(bg_color) };
    unsafe {
        FillRect(mem_dc, &rect, brush);
        let _ = DeleteObject(brush.into());
        SetBkMode(mem_dc, TRANSPARENT);
    }

    // Outer subtle border
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

    let config = AppConfig::load();
    let theme = config.theme();

    let state_guard = UI_STATE.lock().unwrap();
    if let Some(state) = state_guard.as_ref() {
        if state.is_action_menu_open {
            unsafe {
                paint_action_menu_view(mem_dc, state, width, &theme);
            }
        } else {
            match &state.view_mode {
                ViewMode::Search => unsafe {
                    paint_search_view(mem_dc, state, width, &theme);
                },
                ViewMode::Help(content) => unsafe {
                    paint_help_view(
                        mem_dc,
                        content,
                        state.help_scroll_offset,
                        width,
                        height,
                        &theme,
                    );
                },
            }
        }
    }

    unsafe {
        let _ = BitBlt(hdc, 0, 0, width, height, Some(mem_dc), 0, 0, SRCCOPY);
        let _ = SelectObject(mem_dc, old_bmp);
        let _ = DeleteObject(mem_bitmap.into());
        let _ = DeleteDC(mem_dc);
        let _ = EndPaint(hwnd, &ps);
    }
}

unsafe fn paint_search_view(
    hdc: HDC,
    state: &UiState,
    width: i32,
    theme: &crate::config::ThemeColors,
) {
    let hfont_input = create_font(22, 500);
    let old_font = unsafe { SelectObject(hdc, hfont_input.into()) };

    // Mini Logo Icon
    let app_icon = get_app_icon(32);
    unsafe {
        let _ = DrawIconEx(hdc, 20, 23, app_icon, 28, 28, 0, None, DI_NORMAL);
    }

    // Search Input Field
    if state.query.is_empty() {
        unsafe {
            SetTextColor(hdc, rgb(113, 113, 122));
        }
        draw_text(
            hdc,
            "Uygulama, dosya (C:\\), komut (>), dönüştürücü (100km to mi), Tab: Eylemler...",
            62,
            16,
            width - 70,
            58,
            DT_LEFT | DT_VCENTER,
        );
    } else {
        unsafe {
            SetTextColor(hdc, rgb(255, 255, 255));
        }
        draw_text(
            hdc,
            &state.query,
            62,
            16,
            width - 70,
            58,
            DT_LEFT | DT_VCENTER,
        );
    }

    // Top Right Settings Button (⚙️)
    let gear_rect = RECT {
        left: width - 58,
        top: 18,
        right: width - 18,
        bottom: 56,
    };
    let gear_bg = if state.hovered_settings_btn {
        theme.primary_hover
    } else {
        rgb(26, 26, 32)
    };
    let gear_border = if state.hovered_settings_btn {
        theme.primary_border
    } else {
        rgb(48, 48, 56)
    };

    unsafe {
        let b_brush = CreateSolidBrush(gear_border);
        FillRect(hdc, &gear_rect, b_brush);
        let _ = DeleteObject(b_brush.into());

        let inner_g_rect = RECT {
            left: gear_rect.left + 1,
            top: gear_rect.top + 1,
            right: gear_rect.right - 1,
            bottom: gear_rect.bottom - 1,
        };
        let in_brush = CreateSolidBrush(gear_bg);
        FillRect(hdc, &inner_g_rect, in_brush);
        let _ = DeleteObject(in_brush.into());

        let hfont_gear = create_font(18, 500);
        let _ = SelectObject(hdc, hfont_gear.into());
        SetTextColor(
            hdc,
            if state.hovered_settings_btn {
                theme.text_on_primary
            } else {
                rgb(161, 161, 170)
            },
        );
        draw_text(
            hdc,
            "⚙️",
            gear_rect.left,
            gear_rect.top,
            gear_rect.right,
            gear_rect.bottom,
            DT_CENTER | DT_VCENTER,
        );
        let _ = DeleteObject(hfont_gear.into());
    }

    // Divider Line
    let sep_brush = unsafe { CreateSolidBrush(rgb(39, 39, 46)) };
    let sep_rect = RECT {
        left: 18,
        top: 72,
        right: width - 18,
        bottom: 73,
    };
    unsafe {
        FillRect(hdc, &sep_rect, sep_brush);
        let _ = DeleteObject(sep_brush.into());
    }

    let hfont_item = create_font(16, 600);
    let hfont_sub = create_font(12, 500);

    let mut current_y = 78;
    let visible_items: Vec<(usize, &SearchResult)> = state
        .results
        .iter()
        .enumerate()
        .skip(state.scroll_offset)
        .take(VISIBLE_ROWS)
        .collect();

    for (real_index, result) in visible_items.iter() {
        let is_selected = *real_index == state.selected_index;
        let item_rect = RECT {
            left: 14,
            top: current_y,
            right: width - 22,
            bottom: current_y + RESULT_ROW_HEIGHT - 6,
        };

        if is_selected {
            let sel_brush = unsafe { CreateSolidBrush(theme.primary) };
            unsafe {
                FillRect(hdc, &item_rect, sel_brush);
                let _ = DeleteObject(sel_brush.into());
                SetTextColor(hdc, theme.text_on_primary);
            }
        } else {
            unsafe {
                SetTextColor(hdc, rgb(244, 244, 245));
            }
        }

        // Color Preview Swatch (if present)
        let mut text_start_x = 28;
        if let Some(col) = result.color_preview {
            let swatch_rect = RECT {
                left: 28,
                top: current_y + 12,
                right: 52,
                bottom: current_y + 36,
            };
            unsafe {
                let border_brush = CreateSolidBrush(rgb(255, 255, 255));
                FillRect(hdc, &swatch_rect, border_brush);
                let _ = DeleteObject(border_brush.into());

                let inner_swatch = RECT {
                    left: swatch_rect.left + 1,
                    top: swatch_rect.top + 1,
                    right: swatch_rect.right - 1,
                    bottom: swatch_rect.bottom - 1,
                };
                let col_brush = CreateSolidBrush(col);
                FillRect(hdc, &inner_swatch, col_brush);
                let _ = DeleteObject(col_brush.into());
            }
            text_start_x = 60;
        }

        // Item Title
        unsafe {
            let _ = SelectObject(hdc, hfont_item.into());
        }
        draw_text(
            hdc,
            &result.title,
            text_start_x,
            current_y + 4,
            width - 180,
            current_y + RESULT_ROW_HEIGHT - 10,
            DT_LEFT | DT_VCENTER,
        );

        let badge_rect = RECT {
            left: width - 150,
            top: current_y + 10,
            right: width - 34,
            bottom: current_y + RESULT_ROW_HEIGHT - 16,
        };

        let badge_bg = if is_selected {
            theme.primary_hover
        } else {
            rgb(28, 28, 34)
        };
        let badge_border = if is_selected {
            theme.primary_border
        } else {
            rgb(48, 48, 56)
        };

        unsafe {
            let b_brush = CreateSolidBrush(badge_border);
            FillRect(hdc, &badge_rect, b_brush);
            let _ = DeleteObject(b_brush.into());

            let inner_b_rect = RECT {
                left: badge_rect.left + 1,
                top: badge_rect.top + 1,
                right: badge_rect.right - 1,
                bottom: badge_rect.bottom - 1,
            };
            let in_brush = CreateSolidBrush(badge_bg);
            FillRect(hdc, &inner_b_rect, in_brush);
            let _ = DeleteObject(in_brush.into());

            let _ = SelectObject(hdc, hfont_sub.into());
            SetTextColor(
                hdc,
                if is_selected {
                    theme.text_on_primary
                } else {
                    rgb(161, 161, 170)
                },
            );
        }

        draw_text(
            hdc,
            &result.badge,
            badge_rect.left,
            badge_rect.top,
            badge_rect.right,
            badge_rect.bottom,
            DT_CENTER | DT_VCENTER,
        );

        current_y += RESULT_ROW_HEIGHT;
    }

    // Modern Scrollbar Track & Thumb
    if state.results.len() > VISIBLE_ROWS {
        let total_items = state.results.len();
        let max_scroll = (total_items - VISIBLE_ROWS) as i32;
        let track_top = 80;
        let track_height = (VISIBLE_ROWS as i32 * RESULT_ROW_HEIGHT) - 12;
        let thumb_height = ((track_height * VISIBLE_ROWS as i32) / total_items as i32).max(24);

        let thumb_y =
            track_top + (((track_height - thumb_height) * state.scroll_offset as i32) / max_scroll);

        let track_rect = RECT {
            left: width - 12,
            top: track_top,
            right: width - 8,
            bottom: track_top + track_height,
        };

        let thumb_rect = RECT {
            left: width - 12,
            top: thumb_y,
            right: width - 8,
            bottom: thumb_y + thumb_height,
        };

        unsafe {
            let tr_brush = CreateSolidBrush(rgb(32, 32, 38));
            FillRect(hdc, &track_rect, tr_brush);
            let _ = DeleteObject(tr_brush.into());

            let th_brush = CreateSolidBrush(theme.primary_light);
            FillRect(hdc, &thumb_rect, th_brush);
            let _ = DeleteObject(th_brush.into());
        }
    }

    unsafe {
        let _ = SelectObject(hdc, old_font);
        let _ = DeleteObject(hfont_input.into());
        let _ = DeleteObject(hfont_item.into());
        let _ = DeleteObject(hfont_sub.into());
    }
}

unsafe fn paint_help_view(
    hdc: HDC,
    content: &str,
    scroll_offset: i32,
    width: i32,
    height: i32,
    theme: &crate::config::ThemeColors,
) {
    let hfont_title = create_font(20, 700);
    let hfont_body = create_font(14, 400);

    let old_font = unsafe { SelectObject(hdc, hfont_title.into()) };
    let app_icon = get_app_icon(32);
    unsafe {
        let _ = DrawIconEx(hdc, 22, 22, app_icon, 24, 24, 0, None, DI_NORMAL);
        SetTextColor(hdc, rgb(255, 255, 255));
    }

    let header_rect = RECT {
        left: 54,
        top: 18,
        right: width - 24,
        bottom: 50,
    };
    draw_text(
        hdc,
        "LiteCast Kılavuz & Yardım (Esc: Arama Moduna Dön)",
        header_rect.left,
        header_rect.top,
        header_rect.right,
        header_rect.bottom,
        DT_LEFT | DT_VCENTER,
    );

    let sep_brush = unsafe { CreateSolidBrush(rgb(45, 45, 52)) };
    let sep_rect = RECT {
        left: 18,
        top: 56,
        right: width - 18,
        bottom: 57,
    };
    unsafe {
        FillRect(hdc, &sep_rect, sep_brush);
        let _ = DeleteObject(sep_brush.into());
        let _ = SelectObject(hdc, hfont_body.into());
        SetTextColor(hdc, rgb(212, 212, 216));
    }

    let mut text_y = 68 - scroll_offset;
    for line in content.lines() {
        if text_y > -20 && text_y < height + 20 {
            unsafe {
                if line.starts_with('#') {
                    SetTextColor(hdc, theme.primary_light);
                } else {
                    SetTextColor(hdc, rgb(212, 212, 216));
                }
            }
            draw_text(hdc, line, 28, text_y, width - 28, text_y + 22, DT_LEFT);
        }
        text_y += 24;
    }

    unsafe {
        let _ = SelectObject(hdc, old_font);
        let _ = DeleteObject(hfont_title.into());
        let _ = DeleteObject(hfont_body.into());
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

fn draw_text(
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

unsafe fn paint_action_menu_view(
    hdc: HDC,
    state: &UiState,
    width: i32,
    theme: &crate::config::ThemeColors,
) {
    let sel_result = match state.results.get(state.selected_index) {
        Some(r) => r,
        None => return,
    };

    let actions = get_context_actions_for_result(sel_result);

    // Header banner: Selected item summary & Action Menu Title
    let hfont_title = create_font(18, 600);
    let _ = unsafe { SelectObject(hdc, hfont_title.into()) };

    unsafe {
        SetTextColor(hdc, theme.primary_light);
    }
    draw_text(
        hdc,
        &format!("⚡ Eylemler Menüsü: {}", sel_result.title),
        22,
        14,
        width - 160,
        42,
        DT_LEFT | DT_VCENTER,
    );

    // Escape hint pill
    let pill_rect = RECT {
        left: width - 150,
        top: 14,
        right: width - 20,
        bottom: 42,
    };
    unsafe {
        let pill_bg = CreateSolidBrush(rgb(36, 36, 44));
        FillRect(hdc, &pill_rect, pill_bg);
        let _ = DeleteObject(pill_bg.into());
        let hfont_pill = create_font(12, 500);
        let _ = SelectObject(hdc, hfont_pill.into());
        SetTextColor(hdc, rgb(161, 161, 170));
        draw_text(
            hdc,
            "Esc: Geri / Kapat",
            pill_rect.left,
            pill_rect.top,
            pill_rect.right,
            pill_rect.bottom,
            DT_CENTER | DT_VCENTER,
        );
        let _ = DeleteObject(hfont_pill.into());
    }

    // Subtitle
    let hfont_sub = create_font(13, 400);
    let _ = unsafe { SelectObject(hdc, hfont_sub.into()) };
    unsafe {
        SetTextColor(hdc, rgb(140, 140, 150));
    }
    draw_text(
        hdc,
        &format!("Hedef: {}", sel_result.subtitle),
        22,
        40,
        width - 30,
        64,
        DT_LEFT | DT_VCENTER,
    );

    // Separator line
    let sep_brush = unsafe { CreateSolidBrush(rgb(39, 39, 46)) };
    let sep_rect = RECT {
        left: 18,
        top: 70,
        right: width - 18,
        bottom: 71,
    };
    unsafe {
        FillRect(hdc, &sep_rect, sep_brush);
        let _ = DeleteObject(sep_brush.into());
    }

    let hfont_action_title = create_font(16, 600);
    let hfont_action_key = create_font(12, 600);

    let mut current_y = 78;
    for (idx, act_item) in actions.iter().enumerate() {
        let is_selected = idx == state.action_menu_index;
        let row_rect = RECT {
            left: 14,
            top: current_y,
            right: width - 22,
            bottom: current_y + RESULT_ROW_HEIGHT - 6,
        };

        if is_selected {
            let sel_brush = unsafe { CreateSolidBrush(theme.primary) };
            unsafe {
                FillRect(hdc, &row_rect, sel_brush);
                let _ = DeleteObject(sel_brush.into());
                SetTextColor(hdc, theme.text_on_primary);
            }
        } else {
            unsafe {
                SetTextColor(hdc, rgb(240, 240, 245));
            }
        }

        // Action title
        unsafe {
            let _ = SelectObject(hdc, hfont_action_title.into());
        }
        draw_text(
            hdc,
            &format!("{}. {}", idx + 1, act_item.title),
            28,
            current_y + 4,
            width - 200,
            current_y + RESULT_ROW_HEIGHT - 10,
            DT_LEFT | DT_VCENTER,
        );

        // Shortcut badge on right
        let key_badge_rect = RECT {
            left: width - 180,
            top: current_y + 12,
            right: width - 34,
            bottom: current_y + RESULT_ROW_HEIGHT - 18,
        };

        let badge_bg = if is_selected {
            theme.primary_hover
        } else {
            rgb(32, 32, 38)
        };

        unsafe {
            let b_brush = CreateSolidBrush(badge_bg);
            FillRect(hdc, &key_badge_rect, b_brush);
            let _ = DeleteObject(b_brush.into());

            let _ = SelectObject(hdc, hfont_action_key.into());
            SetTextColor(
                hdc,
                if is_selected {
                    theme.text_on_primary
                } else {
                    rgb(161, 161, 170)
                },
            );
            draw_text(
                hdc,
                act_item.key_hint,
                key_badge_rect.left,
                key_badge_rect.top,
                key_badge_rect.right,
                key_badge_rect.bottom,
                DT_CENTER | DT_VCENTER,
            );
        }

        current_y += RESULT_ROW_HEIGHT;
    }

    unsafe {
        let _ = DeleteObject(hfont_title.into());
        let _ = DeleteObject(hfont_sub.into());
        let _ = DeleteObject(hfont_action_title.into());
        let _ = DeleteObject(hfont_action_key.into());
    }
}
