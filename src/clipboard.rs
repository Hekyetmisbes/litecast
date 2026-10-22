// Safe Windows Clipboard Access & History Manager for LiteCast.

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::sync::Mutex;
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CryptProtectData, CryptUnprotectData,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LocalFree(hmem: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
}

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

pub fn dpapi_encrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    unsafe {
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();
        if CryptProtectData(
            &data_in,
            windows::core::PCWSTR::null(),
            None,
            None,
            None,
            0,
            &mut data_out,
        )
        .is_ok()
        {
            let slice = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
            let result = slice.to_vec();
            LocalFree(data_out.pbData as *mut core::ffi::c_void);
            Ok(result)
        } else {
            Err("DPAPI CryptProtectData başarısız".into())
        }
    }
}

pub fn dpapi_decrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    unsafe {
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();
        if CryptUnprotectData(&data_in, None, None, None, None, 0, &mut data_out).is_ok() {
            let slice = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
            let result = slice.to_vec();
            LocalFree(data_out.pbData as *mut core::ffi::c_void);
            Ok(result)
        } else {
            Err("DPAPI CryptUnprotectData başarısız".into())
        }
    }
}

/// Heuristics to detect passwords, tokens, API keys and credit cards
pub fn is_sensitive_content(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }
    let lower = trimmed.to_lowercase();

    // Private keys, JWT, and API tokens
    if lower.contains("begin private key")
        || lower.contains("begin rsa private key")
        || lower.contains("begin openssh private key")
        || lower.contains("bearer eyj")
        || lower.starts_with("ghp_")
        || lower.starts_with("gho_")
        || lower.starts_with("xoxb-")
        || lower.starts_with("sk_live_")
        || lower.starts_with("akias")
    {
        return true;
    }

    // High entropy single token / password heuristic
    if !trimmed.contains(' ')
        && trimmed.len() >= 24
        && trimmed.chars().any(|c| c.is_ascii_digit())
        && trimmed.chars().any(|c| c.is_ascii_uppercase())
        && trimmed.chars().any(|c| c.is_ascii_lowercase())
    {
        return true;
    }

    // Credit card number heuristic (13-19 digits, possibly with space or dash)
    let digits_only: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    if (13..=19).contains(&digits_only.len())
        && (trimmed.contains('-') || trimmed.contains(' '))
        && digits_only.len() == trimmed.replace(['-', ' '], "").len()
    {
        return true;
    }

    false
}

impl ClipboardHistory {
    pub fn encrypted_storage_path() -> PathBuf {
        crate::config::get_app_storage_dir().join("clipboard.enc")
    }

    pub fn legacy_storage_path() -> PathBuf {
        crate::config::get_app_storage_dir().join("clipboard.json")
    }

    pub fn load() -> Self {
        let enc_path = Self::encrypted_storage_path();
        if enc_path.exists() {
            let res = std::fs::read(&enc_path)
                .ok()
                .and_then(|bytes| dpapi_decrypt(&bytes).ok())
                .and_then(|decrypted| serde_json::from_slice::<ClipboardHistory>(&decrypted).ok());
            if let Some(hist) = res {
                return hist;
            }
        }

        // Migrate from legacy unencrypted clipboard.json if exists
        let legacy_path = Self::legacy_storage_path();
        if let Ok(file) = File::open(&legacy_path) {
            let reader = BufReader::new(file);
            if let Ok(hist) = serde_json::from_reader::<_, ClipboardHistory>(reader) {
                let _ = std::fs::remove_file(&legacy_path);
                hist.save();
                return hist;
            }
        }

        Self::default()
    }

    pub fn save(&self) {
        if let Ok(json_bytes) = serde_json::to_vec_pretty(self) {
            if let Ok(encrypted_bytes) = dpapi_encrypt(&json_bytes) {
                let _ = std::fs::write(Self::encrypted_storage_path(), encrypted_bytes);
                // Ensure legacy unencrypted file is removed
                let _ = std::fs::remove_file(Self::legacy_storage_path());
            } else {
                // Fallback safe save
                if let Ok(file) = File::create(Self::legacy_storage_path()) {
                    let writer = BufWriter::new(file);
                    let _ = serde_json::to_writer_pretty(writer, self);
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
        let _ = std::fs::remove_file(Self::encrypted_storage_path());
        let _ = std::fs::remove_file(Self::legacy_storage_path());
    }

    pub fn record(&mut self, raw_text: &str) -> bool {
        let text = raw_text.trim();
        if text.is_empty() || is_sensitive_content(text) {
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

        // Also record in LiteCast history if enabled by user
        let cfg = crate::config::AppConfig::load();
        if cfg.enable_clipboard_history && !is_sensitive_content(text) {
            let mut guard = CLIPBOARD_HISTORY.lock().unwrap();
            if let Some(hist) = guard.as_mut() {
                hist.record(text);
            }
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
    let cfg = crate::config::AppConfig::load();
    if !cfg.enable_clipboard_history {
        return;
    }

    if let Some(text) = get_clipboard_text() {
        if is_sensitive_content(&text) {
            return;
        }
        let mut guard = CLIPBOARD_HISTORY.lock().unwrap();
        if let Some(hist) = guard.as_mut() {
            hist.record(&text);
        }
    }
}
