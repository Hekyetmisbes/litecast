// LiteCast Otomatik Güncelleme & Sunucu Entegrasyonu (Auto-Updater Module)
// Web sitesi veya GitHub üzerinden yeni sürüm denetimi, HTTPS indirme ve kesintisiz hot-swap güncelleme.

use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{AppConfig, get_app_storage_dir};
use crate::platform_windows::to_wide;
use crate::telemetry_local::{log_error, log_info};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_UPDATE_URL: &str =
    "https://raw.githubusercontent.com/castinio/litecast/main/version.json";

#[link(name = "urlmon")]
unsafe extern "system" {
    fn URLDownloadToFileW(
        pcaller: *mut core::ffi::c_void,
        szurl: *const u16,
        szfilename: *const u16,
        dwreserved: u32,
        lpfncb: *mut core::ffi::c_void,
    ) -> i32;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionInfo {
    pub version: String,
    pub download_url: String,
    #[serde(default)]
    pub installer_url: Option<String>,
    #[serde(default)]
    pub release_notes: Option<String>,
    #[serde(default)]
    pub mandatory: bool,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    UpdateAvailable(VersionInfo),
    Downloading(String),
    ReadyToRestart { new_version: String },
    Failed(String),
}

pub static UPDATE_STATUS: Mutex<UpdateStatus> = Mutex::new(UpdateStatus::Idle);
static IS_UPDATING: AtomicBool = AtomicBool::new(false);

/// SemVer version comparison: Returns true if remote is strictly newer than current.
/// Supports "1.0.1" vs "1.0.0", "v1.0.1" vs "1.0.0", etc.
pub fn is_newer_version(remote: &str, current: &str) -> bool {
    let parse_nums = |s: &str| -> Vec<u32> {
        let clean = s.trim().trim_start_matches('v').trim_start_matches('V');
        clean
            .split('.')
            .filter_map(|part| {
                part.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse::<u32>()
                    .ok()
            })
            .collect()
    };

    let r_parts = parse_nums(remote);
    let c_parts = parse_nums(current);

    let max_len = r_parts.len().max(c_parts.len());
    for i in 0..max_len {
        let r = r_parts.get(i).copied().unwrap_or(0);
        let c = c_parts.get(i).copied().unwrap_or(0);
        if r > c {
            return true;
        } else if r < c {
            return false;
        }
    }

    false
}

/// Download a remote HTTP/HTTPS URL directly to a target file path using Windows native urlmon.
pub fn download_file_native(url: &str, dest_path: &Path) -> Result<(), String> {
    if let Some(parent) = dest_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let url_wide = to_wide(url);
    let path_str = dest_path
        .to_str()
        .ok_or_else(|| "Geçersiz dosya yolu".to_string())?;
    let path_wide = to_wide(path_str);

    let hr = unsafe {
        URLDownloadToFileW(
            std::ptr::null_mut(),
            url_wide.as_ptr(),
            path_wide.as_ptr(),
            0,
            std::ptr::null_mut(),
        )
    };

    if hr == 0 {
        if dest_path.exists() && fs::metadata(dest_path).map(|m| m.len()).unwrap_or(0) > 0 {
            Ok(())
        } else {
            Err("İndirilen dosya boş veya kaydedilemedi.".into())
        }
    } else {
        Err(format!(
            "İndirme başarısız oldu (HRESULT: 0x{:08X}). Bağlantıyı kontrol edin.",
            hr
        ))
    }
}

/// Check remote server / website for new version.
pub fn check_for_updates(update_url: &str) -> Result<Option<VersionInfo>, String> {
    let temp_dir = get_app_storage_dir();
    let _ = fs::create_dir_all(&temp_dir);
    let version_file = temp_dir.join("remote_version.json");

    // Remove any stale check
    let _ = fs::remove_file(&version_file);

    download_file_native(update_url, &version_file)?;

    let content =
        fs::read_to_string(&version_file).map_err(|e| format!("Sürüm bilgisi okunamadı: {}", e))?;
    let _ = fs::remove_file(&version_file);

    let info: VersionInfo = serde_json::from_str(&content)
        .map_err(|e| format!("Geçersiz version.json formatı: {}", e))?;

    if is_newer_version(&info.version, CURRENT_VERSION) {
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

/// Downloads the updated binary and executes a hot-swap replacement of the running executable.
pub fn download_and_apply_update(info: &VersionInfo) -> Result<(), String> {
    let storage_dir = get_app_storage_dir();
    let pending_exe = storage_dir.join("update_pending.exe");

    let _ = fs::remove_file(&pending_exe);

    log_info(&format!(
        "Downloading update v{} from {}",
        info.version, info.download_url
    ));
    download_file_native(&info.download_url, &pending_exe)?;

    // Hot-swap the running executable
    let current_exe =
        std::env::current_exe().map_err(|e| format!("Mevcut uygulama konumu alınamadı: {}", e))?;

    let old_exe = current_exe.with_extension("exe.old");

    // 1. Remove previous .old if any
    let _ = fs::remove_file(&old_exe);

    // 2. Rename current running executable to .old (Windows permits rename of running executables)
    fs::rename(&current_exe, &old_exe)
        .map_err(|e| format!("Eski dosya yedeklenemedi (rename): {}", e))?;

    // 3. Move newly downloaded update_pending.exe to original executable path
    if let Err(e) = fs::copy(&pending_exe, &current_exe) {
        // Rollback
        let _ = fs::rename(&old_exe, &current_exe);
        return Err(format!("Yeni sürüm kopyalanamadı: {}", e));
    }

    let _ = fs::remove_file(&pending_exe);
    log_info(&format!(
        "Successfully updated LiteCast to v{}",
        info.version
    ));

    Ok(())
}

/// Cleanup leftover update files from previous runs on startup.
pub fn cleanup_old_update_artifacts() {
    if let Ok(current_exe) = std::env::current_exe() {
        let old_exe = current_exe.with_extension("exe.old");
        if old_exe.exists() {
            let _ = fs::remove_file(&old_exe);
        }
    }

    let storage_dir = get_app_storage_dir();
    let pending_exe = storage_dir.join("update_pending.exe");
    if pending_exe.exists() {
        let _ = fs::remove_file(&pending_exe);
    }
}

/// Starts an asynchronous background worker that checks for updates on startup.
pub fn start_background_update_worker(update_url: String) {
    if IS_UPDATING.swap(true, Ordering::SeqCst) {
        return;
    }

    thread::spawn(move || {
        // Wait 4 seconds after app launch to ensure 0ms immediate startup feel
        thread::sleep(Duration::from_secs(4));

        log_info("Background update check initiated.");
        {
            let mut status = UPDATE_STATUS.lock().unwrap();
            *status = UpdateStatus::Checking;
        }

        match check_for_updates(&update_url) {
            Ok(Some(info)) => {
                log_info(&format!(
                    "New update available: v{} (Current: v{})",
                    info.version, CURRENT_VERSION
                ));

                {
                    let mut status = UPDATE_STATUS.lock().unwrap();
                    *status = UpdateStatus::UpdateAvailable(info.clone());
                }

                // If auto-update is active, proceed with hot-swap download
                let config = AppConfig::load();
                if config.auto_update {
                    {
                        let mut status = UPDATE_STATUS.lock().unwrap();
                        *status = UpdateStatus::Downloading(info.version.clone());
                    }

                    match download_and_apply_update(&info) {
                        Ok(_) => {
                            let mut status = UPDATE_STATUS.lock().unwrap();
                            *status = UpdateStatus::ReadyToRestart {
                                new_version: info.version.clone(),
                            };
                            log_info("Update ready: Will apply on next restart.");
                        }
                        Err(e) => {
                            log_error(&format!("Auto-update download failed: {}", e));
                            let mut status = UPDATE_STATUS.lock().unwrap();
                            *status = UpdateStatus::Failed(e);
                        }
                    }
                }
            }
            Ok(None) => {
                log_info("LiteCast is up to date.");
                let mut status = UPDATE_STATUS.lock().unwrap();
                *status = UpdateStatus::UpToDate;
            }
            Err(e) => {
                log_info(&format!("Update check failed / offline: {}", e));
                let mut status = UPDATE_STATUS.lock().unwrap();
                *status = UpdateStatus::Failed(e);
            }
        }

        IS_UPDATING.store(false, Ordering::SeqCst);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("1.0.1", "1.0.0"));
        assert!(is_newer_version("1.1.0", "1.0.9"));
        assert!(is_newer_version("2.0.0", "1.99.99"));
        assert!(is_newer_version("v1.0.1", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "1.0.1"));
        assert!(!is_newer_version("0.9.9", "1.0.0"));
    }

    #[test]
    fn test_parse_version_json() {
        let sample = r#"{
            "version": "1.0.1",
            "download_url": "https://example.com/LiteCast.exe",
            "installer_url": "https://example.com/LiteCast-Setup.exe",
            "release_notes": "Süper yenilikler!",
            "mandatory": false,
            "published_at": "2026-09-01T18:00:00Z"
        }"#;

        let info: Result<VersionInfo, _> = serde_json::from_str(sample);
        assert!(info.is_ok());
        let info = info.unwrap();
        assert_eq!(info.version, "1.0.1");
        assert_eq!(info.download_url, "https://example.com/LiteCast.exe");
        assert_eq!(info.release_notes.as_deref(), Some("Süper yenilikler!"));
    }
}
