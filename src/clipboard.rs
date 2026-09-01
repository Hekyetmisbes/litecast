// Safe Windows Clipboard Access & History Manager for LiteCast.

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::sync::Mutex;
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};

pub const CF_UNICODETEXT: u32 = 13;
pub const MAX_CLIPBOARD_ITEMS: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClipboardItem {
    pub id: String,
    pub text: String,
    pub preview: String,
    pub char_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClipboardHistory {
    pub items: Vec<ClipboardItem>,
}

pub static CLIPBOARD_HISTORY: Mutex<Option<ClipboardHistory>> = Mutex::new(None);

impl ClipboardHistory {
    pub fn storage_path() -> PathBuf {
        crate::config::get_app_storage_dir().join("clipboard.json")
    }

    pub fn load() -> Self {
        if let Ok(file) = File::open(Self::storage_path()) {
            let reader = BufReader::new(file);
            if let Ok(hist) = serde_json::from_reader(reader) {
                return hist;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Ok(file) = File::create(Self::storage_path()) {
            let writer = BufWriter::new(file);
            let _ = serde_json::to_writer_pretty(writer, self);
        }
    }

    pub fn record(&mut self, raw_text: &str) -> bool {
        let text = raw_text.trim();
        if text.is_empty() {
            return false;
        }

        // Remove existing duplicate
        self.items.retain(|item| item.text != text);

        let clean_preview = text.lines().next().unwrap_or("").trim().to_string();
        let preview = if clean_preview.len() > 60 {
            format!("{}...", &clean_preview[..57])
        } else {
            clean_preview
        };

        let item = ClipboardItem {
            id: format!(
                "clip_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            ),
            text: text.to_string(),
            preview: if preview.is_empty() {
                "Metin Panosu".into()
            } else {
                preview
            },
            char_count: text.chars().count(),
        };

        self.items.insert(0, item);
        if self.items.len() > MAX_CLIPBOARD_ITEMS {
            self.items.truncate(MAX_CLIPBOARD_ITEMS);
        }

        self.save();
        true
    }

    pub fn search(&self, query: &str) -> Vec<ClipboardItem> {
        let q = query.trim().to_lowercase();
        if q.is_empty() || q == "cb" || q == "clip" || q == "pano" {
            return self.items.clone();
        }

        self.items
            .iter()
            .filter(|item| item.text.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }
}

pub fn get_clipboard_text() -> Option<String> {
    unsafe {
        if OpenClipboard(Some(HWND::default())).is_err() {
            return None;
        }

        if IsClipboardFormatAvailable(CF_UNICODETEXT).is_err() {
            let _ = CloseClipboard();
            return None;
        }

        let handle_res = GetClipboardData(CF_UNICODETEXT);
        if handle_res.is_err() {
            let _ = CloseClipboard();
            return None;
        }

        let handle = handle_res.unwrap();
        if handle.0.is_null() {
            let _ = CloseClipboard();
            return None;
        }

        let ptr = GlobalLock(HGLOBAL(handle.0));
        if ptr.is_null() {
            let _ = CloseClipboard();
            return None;
        }

        let wide_slice = std::slice::from_raw_parts(ptr as *const u16, usize::MAX / 2);
        let mut len = 0;
        while len < 100_000 && wide_slice[len] != 0 {
            len += 1;
        }

        let text = String::from_utf16_lossy(&wide_slice[..len]);
        let _ = GlobalUnlock(HGLOBAL(handle.0));
        let _ = CloseClipboard();

        Some(text)
    }
}

pub fn set_clipboard_text(text: &str) -> Result<(), String> {
    unsafe {
        if OpenClipboard(Some(HWND::default())).is_err() {
            return Err("Cannot open clipboard".into());
        }

        let _ = EmptyClipboard();

        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes_len = wide.len() * std::mem::size_of::<u16>();

        let hmem = match GlobalAlloc(GMEM_MOVEABLE, bytes_len) {
            Ok(h) => h,
            Err(e) => {
                let _ = CloseClipboard();
                return Err(format!("GlobalAlloc failed: {}", e));
            }
        };

        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err("GlobalLock failed".into());
        }

        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, bytes_len);
        let _ = GlobalUnlock(hmem);

        if SetClipboardData(CF_UNICODETEXT, Some(HANDLE(hmem.0))).is_err() {
            let _ = CloseClipboard();
            return Err("SetClipboardData failed".into());
        }

        let _ = CloseClipboard();

        // Also record in LiteCast history
        let mut guard = CLIPBOARD_HISTORY.lock().unwrap();
        if let Some(hist) = guard.as_mut() {
            hist.record(text);
        }

        Ok(())
    }
}

pub fn init_clipboard_manager() {
    let hist = ClipboardHistory::load();
    let mut guard = CLIPBOARD_HISTORY.lock().unwrap();
    *guard = Some(hist);
}

pub fn check_and_record_current_clipboard() {
    if let Some(text) = get_clipboard_text() {
        let mut guard = CLIPBOARD_HISTORY.lock().unwrap();
        if let Some(hist) = guard.as_mut() {
            hist.record(&text);
        }
    }
}
