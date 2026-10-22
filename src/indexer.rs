use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RegCloseKey, RegEnumKeyExW,
    RegOpenKeyExW, RegQueryValueExW,
};
use windows::core::{PCWSTR, PWSTR};

use crate::platform_windows::to_wide;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppItem {
    pub id: String,
    pub name: String,
    pub target_path: String,
    pub arguments: Option<String>,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppIndex {
    pub items: Vec<AppItem>,
}

impl AppIndex {
    pub fn cache_path() -> PathBuf {
        crate::config::get_app_storage_dir().join("cache.json")
    }

    pub fn load_or_rebuild() -> Self {
        if let Ok(file) = File::open(Self::cache_path()) {
            let reader = BufReader::new(file);
            if let Ok(cached) = serde_json::from_reader::<_, AppIndex>(reader) {
                let has_uwp = cached.items.iter().any(|a| {
                    a.source_path.starts_with("uwp:")
                        || a.source_path.starts_with("startapp:")
                        || a.target_path.starts_with("shell:AppsFolder\\")
                });
                if !cached.items.is_empty() && has_uwp {
                    return cached;
                }
            }
        }
        Self::rebuild()
    }

    pub fn rebuild() -> Self {
        Self::rebuild_with_progress(|_, _, _| {})
    }

    pub fn rebuild_with_progress<F>(on_progress: F) -> Self
    where
        F: FnMut(f32, &str, usize),
    {
        let items = discover_applications_with_progress(on_progress);
        let index = Self { items };
        index.save();
        index
    }

    pub fn save(&self) {
        if let Ok(file) = File::create(Self::cache_path()) {
            let writer = BufWriter::new(file);
            let _ = serde_json::to_writer(writer, self);
        }
    }
}

pub fn discover_applications() -> Vec<AppItem> {
    discover_applications_with_progress(|_, _, _| {})
}

pub fn discover_applications_with_progress<F>(mut on_progress: F) -> Vec<AppItem>
where
    F: FnMut(f32, &str, usize),
{
    let mut apps = Vec::new();

    // 1. Built-in Windows System Tools
    on_progress(0.08, "Sistem araçları taranıyor...", apps.len());
    add_system_tools(&mut apps);

    // 2. Windows StartApps & Microsoft Store / UWP Applications (ChatGPT, Terminal, WhatsApp, etc.)
    on_progress(
        0.20,
        "Windows Start & Mağaza uygulamaları taranıyor...",
        apps.len(),
    );
    add_start_apps(&mut apps);

    // 3. Steam Games & Libraries
    on_progress(0.35, "Steam oyun kütüphaneleri taranıyor...", apps.len());
    add_steam_games(&mut apps);

    // 4. Epic Games Store & Game Launchers
    on_progress(
        0.48,
        "Epic, Riot ve oyun kütüphaneleri taranıyor...",
        apps.len(),
    );
    add_epic_games(&mut apps);
    add_riot_games(&mut apps);
    add_xbox_games(&mut apps);
    add_gog_games(&mut apps);
    add_ubisoft_games(&mut apps);
    add_custom_games_folders(&mut apps);

    // 10. Windows Registry Installed Software & Games
    on_progress(
        0.68,
        "Windows Kayıt Defteri (Registry) taranıyor...",
        apps.len(),
    );
    add_registry_installed_apps(&mut apps);

    // 11. Known directories to scan for .lnk / .url / .exe
    on_progress(0.82, "Başlat Menüsü ve Masaüstü taranıyor...", apps.len());
    let mut search_dirs = Vec::new();

    if let Ok(app_data) = std::env::var("APPDATA") {
        search_dirs.push(PathBuf::from(&app_data).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Ok(prog_data) = std::env::var("ProgramData") {
        search_dirs.push(PathBuf::from(&prog_data).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        search_dirs.push(PathBuf::from(&local_app_data).join("Programs"));
        search_dirs.push(PathBuf::from(&local_app_data).join(r"Microsoft\WindowsApps"));
    }
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        search_dirs.push(PathBuf::from(&user_profile).join("Desktop"));
    }
    if let Ok(public_dir) = std::env::var("PUBLIC") {
        search_dirs.push(PathBuf::from(&public_dir).join("Desktop"));
    }

    for dir in search_dirs {
        if dir.exists() {
            scan_dir_for_apps(&dir, &mut apps, 0);
        }
    }

    // 12. Filter out junk and uninstaller entries
    apps.retain(|a| !is_junk_app(&a.name));

    // Deduplicate by lowercased name, preferring items with launcher URIs / clean targets
    on_progress(0.95, "Yinelenen kayıtlar optimize ediliyor...", apps.len());
    apps.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    apps.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));

    on_progress(1.00, "İndeksleme tamamlandı!", apps.len());
    apps
}

pub fn is_junk_app(name: &str) -> bool {
    let lower = name.trim().to_lowercase();
    if lower.is_empty() {
        return true;
    }
    lower.starts_with("unins000")
        || lower.starts_with("uninstall")
        || lower.starts_with("kaldır")
        || lower.starts_with("yardım")
        || lower.starts_with("help")
        || lower.starts_with("update for")
        || lower.starts_with("security update")
        || lower.starts_with("vs_")
        || lower.ends_with("yardımı")
        || lower.ends_with("documentation")
        || lower.ends_with("release notes")
        || lower.ends_with("reset preferences and cache files")
        || lower.ends_with("read me")
        || lower.ends_with("readme")
        || lower.contains("redistributable")
        || lower.contains("software development kit")
        || lower.contains("windows sdk")
        || lower.contains("icecap_collection")
        || lower.contains("winappdeploy")
}

// -----------------------------------------------------------------------------
// Win32 Registry Helpers
// -----------------------------------------------------------------------------

pub fn read_reg_string(root: HKEY, subkey: &str, value_name: &str) -> Option<String> {
    unsafe {
        let wide_sub = to_wide(subkey);
        let wide_val = to_wide(value_name);
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            root,
            PCWSTR::from_raw(wide_sub.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        )
        .is_ok()
        {
            let mut buf = [0u16; 1024];
            let mut size = (buf.len() * 2) as u32;
            let res = RegQueryValueExW(
                hkey,
                PCWSTR::from_raw(wide_val.as_ptr()),
                None,
                None,
                Some(buf.as_mut_ptr() as *mut u8),
                Some(&mut size),
            );
            let _ = RegCloseKey(hkey);
            if res.is_ok() && size > 1 {
                let len = ((size as usize) / 2).saturating_sub(1);
                let s = String::from_utf16_lossy(&buf[..len]);
                let trimmed = s.trim_matches('\0').trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    }
}

pub fn enum_reg_subkeys(root: HKEY, subkey: &str) -> Vec<String> {
    let mut subkeys = Vec::new();
    unsafe {
        let wide_sub = to_wide(subkey);
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            root,
            PCWSTR::from_raw(wide_sub.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        )
        .is_ok()
        {
            let mut index = 0u32;
            loop {
                let mut name_buf = [0u16; 256];
                let mut name_len = name_buf.len() as u32;
                let res = RegEnumKeyExW(
                    hkey,
                    index,
                    Some(PWSTR(name_buf.as_mut_ptr())),
                    &mut name_len,
                    None,
                    None,
                    None,
                    None,
                );
                if res.is_ok() {
                    let s = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                    if !s.is_empty() {
                        subkeys.push(s);
                    }
                    index += 1;
                } else {
                    break;
                }
            }
            let _ = RegCloseKey(hkey);
        }
    }
    subkeys
}

// -----------------------------------------------------------------------------
// 1. Steam Discovery
// -----------------------------------------------------------------------------

pub fn add_steam_games(apps: &mut Vec<AppItem>) {
    let mut steam_paths = Vec::new();

    if let Some(p) = read_reg_string(HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath") {
        steam_paths.push(PathBuf::from(p));
    }
    if let Some(p) = read_reg_string(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\WOW6432Node\Valve\Steam",
        "InstallPath",
    ) {
        steam_paths.push(PathBuf::from(p));
    }
    if let Some(p) = read_reg_string(HKEY_LOCAL_MACHINE, r"SOFTWARE\Valve\Steam", "InstallPath") {
        steam_paths.push(PathBuf::from(p));
    }

    // Default fallbacks
    steam_paths.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    steam_paths.push(PathBuf::from(r"C:\Program Files\Steam"));
    steam_paths.push(PathBuf::from(r"D:\Steam"));
    steam_paths.push(PathBuf::from(r"D:\SteamLibrary"));

    let mut libraries = Vec::new();

    for steam_dir in steam_paths {
        if !steam_dir.exists() {
            continue;
        }
        if !libraries.contains(&steam_dir) {
            libraries.push(steam_dir.clone());
        }

        // Parse libraryfolders.vdf
        let vdf_path = steam_dir.join(r"steamapps\libraryfolders.vdf");
        if vdf_path.exists()
            && let Ok(file) = File::open(&vdf_path)
        {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if trimmed.starts_with("\"path\"") {
                    // Extract path between remaining quotes
                    let parts: Vec<&str> = trimmed.split('"').collect();
                    if parts.len() >= 4 {
                        let path_str = parts[3].replace(r"\\", r"\");
                        let lib_path = PathBuf::from(path_str);
                        if lib_path.exists() && !libraries.contains(&lib_path) {
                            libraries.push(lib_path);
                        }
                    }
                }
            }
        }
    }

    // Scan all steamapps folders for appmanifest_*.acf
    for lib in libraries {
        let steamapps = lib.join("steamapps");
        if steamapps.exists()
            && let Ok(entries) = std::fs::read_dir(steamapps)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str())
                    && name.starts_with("appmanifest_")
                    && name.ends_with(".acf")
                    && let Some((appid, game_name)) = parse_steam_manifest(&path)
                {
                    // Filter common Steam redistributables / non-games
                    if appid == "228980"
                        || game_name.is_empty()
                        || game_name
                            .to_lowercase()
                            .contains("steamworks common redistributables")
                        || game_name.to_lowercase().contains("steam linux runtime")
                        || game_name.to_lowercase().starts_with("proton ")
                        || game_name.to_lowercase().ends_with(" dedicated server")
                    {
                        continue;
                    }

                    apps.push(AppItem {
                        id: format!("steam_{}", appid),
                        name: game_name,
                        target_path: format!("steam://rungameid/{}", appid),
                        arguments: None,
                        source_path: path.to_string_lossy().to_string(),
                    });
                }
            }
        }
    }
}

fn parse_steam_manifest(path: &Path) -> Option<(String, String)> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut appid: Option<String> = None;
    let mut name: Option<String> = None;

    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if appid.is_none() && trimmed.starts_with("\"appid\"") {
            let parts: Vec<&str> = trimmed.split('"').collect();
            if parts.len() >= 4 {
                appid = Some(parts[3].to_string());
            }
        }
        if name.is_none() && trimmed.starts_with("\"name\"") {
            let parts: Vec<&str> = trimmed.split('"').collect();
            if parts.len() >= 4 {
                name = Some(parts[3].to_string());
            }
        }
        if appid.is_some() && name.is_some() {
            break;
        }
    }

    match (appid, name) {
        (Some(id), Some(n)) if !id.is_empty() && !n.is_empty() => Some((id, n)),
        _ => None,
    }
}

// -----------------------------------------------------------------------------
// 2. Epic Games Store Discovery
// -----------------------------------------------------------------------------

pub fn add_epic_games(apps: &mut Vec<AppItem>) {
    let epic_manifests_dirs = vec![
        PathBuf::from(r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests"),
        PathBuf::from(r"D:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests"),
    ];

    for dir in epic_manifests_dirs {
        if dir.exists()
            && let Ok(entries) = std::fs::read_dir(dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("item")
                    && let Ok(file) = File::open(&path)
                {
                    let reader = BufReader::new(file);
                    if let Ok(val) = serde_json::from_reader::<_, serde_json::Value>(reader) {
                        let display_name = val["DisplayName"].as_str().unwrap_or("").trim();
                        let app_name = val["AppName"].as_str().unwrap_or("").trim();
                        let install_loc = val["InstallLocation"].as_str().unwrap_or("").trim();

                        if !display_name.is_empty()
                            && !app_name.is_empty()
                            && (install_loc.is_empty() || Path::new(install_loc).exists())
                        {
                            // Filter plugins without executables (e.g. Fab UE Plugin)
                            let launch_exe = val["LaunchExecutable"].as_str().unwrap_or("").trim();
                            if launch_exe.is_empty()
                                && display_name.to_lowercase().contains("plugin")
                            {
                                continue;
                            }

                            apps.push(AppItem {
                                id: format!("epic_{}", app_name),
                                name: display_name.to_string(),
                                target_path: format!(
                                    "com.epicgames.launcher://apps/{}?action=launch&silent=true",
                                    app_name
                                ),
                                arguments: None,
                                source_path: path.to_string_lossy().to_string(),
                            });
                        }
                    }
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// 3. Riot Games Discovery
// -----------------------------------------------------------------------------

pub fn add_riot_games(apps: &mut Vec<AppItem>) {
    let mut riot_client_exe: Option<String> = None;

    // Try finding RiotClientServices.exe from installs json
    let inst_path = PathBuf::from(r"C:\ProgramData\Riot Games\RiotClientInstalls.json");
    if inst_path.exists()
        && let Ok(file) = File::open(&inst_path)
    {
        let reader = BufReader::new(file);
        if let Ok(json) = serde_json::from_reader::<_, serde_json::Value>(reader)
            && let Some(rc) = json["rc_live"]
                .as_str()
                .or_else(|| json["rc_default"].as_str())
            && Path::new(rc).exists()
        {
            riot_client_exe = Some(rc.to_string());
        }
    }

    if riot_client_exe.is_none() {
        let fallback_rc = PathBuf::from(r"C:\Riot Games\Riot Client\RiotClientServices.exe");
        if fallback_rc.exists() {
            riot_client_exe = Some(fallback_rc.to_string_lossy().to_string());
        }
    }

    // Check Metadata directory for installed products
    let meta_dir = PathBuf::from(r"C:\ProgramData\Riot Games\Metadata");
    if meta_dir.exists()
        && let Ok(entries) = std::fs::read_dir(meta_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let product_id = if dir_name.contains("valorant") {
                    "valorant"
                } else if dir_name.contains("league_of_legends") {
                    "league_of_legends"
                } else if dir_name.contains("bacon") {
                    "bacon"
                } else if dir_name.contains("tft") {
                    "tft"
                } else {
                    continue;
                };

                let display_name = match product_id {
                    "valorant" => "VALORANT",
                    "league_of_legends" => "League of Legends",
                    "bacon" => "Legends of Runeterra",
                    "tft" => "Teamfight Tactics",
                    _ => product_id,
                };

                let Some(ref rc) = riot_client_exe else {
                    continue;
                };

                let target_path = rc.clone();
                let arguments = Some(format!(
                    "--launch-product={} --launch-patchline=live",
                    product_id
                ));

                apps.push(AppItem {
                    id: format!("riot_{}", product_id),
                    name: display_name.to_string(),
                    target_path,
                    arguments,
                    source_path: path.to_string_lossy().to_string(),
                });
            }
        }
    }
}

// -----------------------------------------------------------------------------
// 4. Xbox & PC Game Pass Games
// -----------------------------------------------------------------------------

pub fn add_xbox_games(apps: &mut Vec<AppItem>) {
    let drives = ['C', 'D', 'E', 'F', 'G'];
    for &d in &drives {
        let xbox_dir = PathBuf::from(format!(r"{}:\XboxGames", d));
        if !xbox_dir.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(xbox_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let folder_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if folder_name.is_empty() || folder_name.eq_ignore_ascii_case("GameSave") {
                        continue;
                    }

                    // Look for gamelaunchhelper.exe or main .exe in root or Content
                    if let Some(exe) = find_primary_exe_in_game_dir(&path) {
                        apps.push(AppItem {
                            id: format!("xbox_{}", folder_name.to_lowercase().replace(' ', "_")),
                            name: folder_name.to_string(),
                            target_path: exe.to_string_lossy().to_string(),
                            arguments: None,
                            source_path: path.to_string_lossy().to_string(),
                        });
                    }
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// 5. GOG Galaxy Games
// -----------------------------------------------------------------------------

pub fn add_gog_games(apps: &mut Vec<AppItem>) {
    let subkeys = enum_reg_subkeys(HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\GOG.com\Games");
    for sub in subkeys {
        let sub_path = format!(r"SOFTWARE\WOW6432Node\GOG.com\Games\{}", sub);
        let name = read_reg_string(HKEY_LOCAL_MACHINE, &sub_path, "gameName")
            .or_else(|| read_reg_string(HKEY_LOCAL_MACHINE, &sub_path, "GAMENAME"));
        let path_str = read_reg_string(HKEY_LOCAL_MACHINE, &sub_path, "path")
            .or_else(|| read_reg_string(HKEY_LOCAL_MACHINE, &sub_path, "PATH"));
        let exe = read_reg_string(HKEY_LOCAL_MACHINE, &sub_path, "exe")
            .or_else(|| read_reg_string(HKEY_LOCAL_MACHINE, &sub_path, "EXE"));

        if let (Some(game_name), Some(dir)) = (name, path_str) {
            let full_target = if let Some(e) = exe {
                let p = PathBuf::from(&dir).join(e);
                if p.exists() {
                    p.to_string_lossy().to_string()
                } else {
                    format!("goggalaxy://openGameView/{}", sub)
                }
            } else {
                format!("goggalaxy://openGameView/{}", sub)
            };

            apps.push(AppItem {
                id: format!("gog_{}", sub),
                name: game_name,
                target_path: full_target,
                arguments: None,
                source_path: sub_path,
            });
        }
    }
}

// -----------------------------------------------------------------------------
// 6. Ubisoft Connect Games
// -----------------------------------------------------------------------------

pub fn add_ubisoft_games(apps: &mut Vec<AppItem>) {
    let subkeys = enum_reg_subkeys(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs",
    );
    for sub in subkeys {
        let sub_path = format!(r"SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\{}", sub);
        if let Some(install_dir) = read_reg_string(HKEY_LOCAL_MACHINE, &sub_path, "InstallDir") {
            let dir_path = PathBuf::from(install_dir);
            if dir_path.exists() {
                let folder_name = dir_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&sub)
                    .replace('_', " ");

                let target = if let Some(exe) = find_primary_exe_in_game_dir(&dir_path) {
                    exe.to_string_lossy().to_string()
                } else {
                    format!("uplay://launch/{}/0", sub)
                };

                apps.push(AppItem {
                    id: format!("ubisoft_{}", sub),
                    name: folder_name,
                    target_path: target,
                    arguments: None,
                    source_path: sub_path,
                });
            }
        }
    }
}

// -----------------------------------------------------------------------------
// 7. Custom Game Directories (D:\Oyunlar, C:\Games, D:\Games, etc.)
// -----------------------------------------------------------------------------

pub fn add_custom_games_folders(apps: &mut Vec<AppItem>) {
    let drives = ['C', 'D', 'E', 'F', 'G'];
    let folders = ["Oyunlar", "Games", "Oyun", "Game"];

    for &d in &drives {
        for &folder in &folders {
            let root = PathBuf::from(format!(r"{}:\{}", d, folder));
            if !root.exists() {
                continue;
            }

            if let Ok(entries) = std::fs::read_dir(&root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        let lower_name = dir_name.to_lowercase();
                        // Skip sub-launchers already indexed specifically
                        if lower_name == "riot games"
                            || lower_name == "steam"
                            || lower_name == "epic games"
                            || lower_name == "ubisoft"
                        {
                            continue;
                        }

                        if let Some(exe) = find_primary_exe_in_game_dir(&path) {
                            let display_name = dir_name.replace('_', " ");
                            apps.push(AppItem {
                                id: format!("game_{}", path.to_string_lossy()),
                                name: display_name,
                                target_path: exe.to_string_lossy().to_string(),
                                arguments: None,
                                source_path: path.to_string_lossy().to_string(),
                            });
                        }
                    }
                }
            }
        }
    }
}

fn find_primary_exe_in_game_dir(dir: &Path) -> Option<PathBuf> {
    // Check standard candidate subdirectories
    let subdirs = [
        dir.to_path_buf(),
        dir.join("bin"),
        dir.join("bin_plus"),
        dir.join("bin64"),
        dir.join("win64"),
        dir.join("x64"),
        dir.join("Content"),
        dir.join("Game"),
        dir.join(r"Binaries\Win64"),
    ];

    let mut candidate_exes = Vec::new();

    for sub in subdirs {
        if !sub.exists() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(sub) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("exe") {
                    let stem = p
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    // Filter helper / junk tools
                    if stem.starts_with("unins")
                        || stem.starts_with("uninstall")
                        || stem.starts_with("crash")
                        || stem.starts_with("unitycrash")
                        || stem.starts_with("eac")
                        || stem.starts_with("easyanticheat")
                        || stem.starts_with("splash")
                        || stem.starts_with("setup")
                        || stem.starts_with("dxsetup")
                        || stem.starts_with("vcredist")
                        || stem.starts_with("support")
                        || stem.starts_with("redist")
                        || stem.contains("report")
                    {
                        continue;
                    }

                    if stem == "gamelaunchhelper" {
                        return Some(p);
                    }

                    candidate_exes.push(p);
                }
            }
        }
    }

    let dir_stem = dir
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
        .replace('_', "");

    // Prioritize exe whose name matches game directory stem
    for exe in &candidate_exes {
        let exe_stem = exe
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase()
            .replace('_', "");
        if !exe_stem.is_empty() && (exe_stem == dir_stem || dir_stem.contains(&exe_stem)) {
            return Some(exe.clone());
        }
    }

    candidate_exes.into_iter().next()
}

// -----------------------------------------------------------------------------
// 8. Windows Registry Installed Apps & Standalone Games
// -----------------------------------------------------------------------------

pub fn add_registry_installed_apps(apps: &mut Vec<AppItem>) {
    let registry_roots = [
        (
            HKEY_LOCAL_MACHINE,
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"Software\Wow6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_CURRENT_USER,
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ];

    for (root, subkey_base) in registry_roots {
        let subkeys = enum_reg_subkeys(root, subkey_base);
        for sub in subkeys {
            let sub_path = format!(r"{}\{}", subkey_base, sub);
            let display_name = read_reg_string(root, &sub_path, "DisplayName");
            if let Some(name) = display_name {
                let trimmed_name = name.trim();
                if trimmed_name.is_empty() || is_junk_app(trimmed_name) {
                    continue;
                }

                // Check DisplayIcon
                if let Some(icon) = read_reg_string(root, &sub_path, "DisplayIcon") {
                    let clean_icon = icon
                        .split(',')
                        .next()
                        .unwrap_or("")
                        .trim_matches('"')
                        .trim();
                    let icon_path = PathBuf::from(clean_icon);
                    if icon_path.exists()
                        && icon_path.extension().and_then(|e| e.to_str()) == Some("exe")
                    {
                        let stem = icon_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        if !stem.starts_with("unins") && !stem.starts_with("uninstall") {
                            apps.push(AppItem {
                                id: format!("reg_{}", sub),
                                name: trimmed_name.to_string(),
                                target_path: icon_path.to_string_lossy().to_string(),
                                arguments: None,
                                source_path: sub_path.clone(),
                            });
                            continue;
                        }
                    }
                }

                // Check InstallLocation
                if let Some(loc) = read_reg_string(root, &sub_path, "InstallLocation") {
                    let clean_loc = loc.trim_matches('"').trim();
                    let loc_path = PathBuf::from(clean_loc);
                    if loc_path.exists()
                        && let Some(exe) = find_primary_exe_in_game_dir(&loc_path)
                    {
                        apps.push(AppItem {
                            id: format!("reg_{}", sub),
                            name: trimmed_name.to_string(),
                            target_path: exe.to_string_lossy().to_string(),
                            arguments: None,
                            source_path: sub_path,
                        });
                    }
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Existing StartApps, System Tools, and Scan Dir
// -----------------------------------------------------------------------------

fn add_start_apps(apps: &mut Vec<AppItem>) {
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; @(Get-StartApps) | Select-Object Name, AppID | ConvertTo-Json -Compress",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    if let Ok(out) = output
        && out.status.success()
    {
        let json_str = String::from_utf8_lossy(&out.stdout);
        let trimmed = json_str.trim();
        if !trimmed.is_empty() {
            #[derive(Deserialize)]
            struct RawApp {
                #[serde(rename = "Name")]
                name: Option<String>,
                #[serde(rename = "AppID")]
                app_id: Option<String>,
            }

            let raw_list: Vec<RawApp> =
                if let Ok(list) = serde_json::from_str::<Vec<RawApp>>(trimmed) {
                    list
                } else if let Ok(single) = serde_json::from_str::<RawApp>(trimmed) {
                    vec![single]
                } else {
                    Vec::new()
                };

            for item in raw_list {
                let name = item.name.unwrap_or_default().trim().to_string();
                let app_id = item.app_id.unwrap_or_default().trim().to_string();
                if name.is_empty() || app_id.is_empty() || is_junk_app(&name) {
                    continue;
                }
                let target_path = if app_id.starts_with("http://") || app_id.starts_with("https://")
                {
                    app_id.clone()
                } else if app_id.contains('!') || !app_id.ends_with(".exe") {
                    format!("shell:AppsFolder\\{}", app_id)
                } else {
                    app_id.clone()
                };

                let source_path = if app_id.contains('!') {
                    format!("uwp:{}", app_id)
                } else {
                    format!("startapp:{}", app_id)
                };

                apps.push(AppItem {
                    id: format!("start_{}", app_id),
                    name,
                    target_path,
                    arguments: None,
                    source_path,
                });
            }
        }
    }
}

fn add_system_tools(apps: &mut Vec<AppItem>) {
    let tools = [
        (
            "Denetim Masası",
            "control.exe",
            "Denetim Masası (Control Panel)",
        ),
        ("Control Panel", "control.exe", "Control Panel"),
        ("Ayarlar", "ms-settings:", "Windows Ayarları (Settings)"),
        ("Settings", "ms-settings:", "Windows Settings"),
        (
            "Görev Yöneticisi",
            "taskmgr.exe",
            "Görev Yöneticisi (Task Manager)",
        ),
        ("Task Manager", "taskmgr.exe", "Task Manager"),
        ("Hesap Makinesi", "calc.exe", "Hesap Makinesi"),
        ("Calculator", "calc.exe", "Calculator"),
        ("Not Defteri", "notepad.exe", "Not Defteri (Notepad)"),
        ("Notepad", "notepad.exe", "Notepad"),
        ("Komut İstemi", "cmd.exe", "Komut İstemi (Command Prompt)"),
        ("Command Prompt", "cmd.exe", "Command Prompt"),
        ("PowerShell", "powershell.exe", "Windows PowerShell"),
        ("Dosya Gezgini", "explorer.exe", "Dosya Gezgini"),
        ("File Explorer", "explorer.exe", "File Explorer"),
        (
            "Kayıt Defteri Düzenleyicisi",
            "regedit.exe",
            "Kayıt Defteri (Regedit)",
        ),
        ("Registry Editor", "regedit.exe", "Registry Editor"),
        (
            "Aygıt Yöneticisi",
            "devmgmt.msc",
            "Aygıt Yöneticisi (Device Manager)",
        ),
        ("Device Manager", "devmgmt.msc", "Device Manager"),
        (
            "Disk Yönetimi",
            "diskmgmt.msc",
            "Disk Yönetimi (Disk Management)",
        ),
        ("Disk Management", "diskmgmt.msc", "Disk Management"),
        ("Hizmetler", "services.msc", "Hizmetler (Services)"),
        ("Services", "services.msc", "Services"),
        (
            "Ekran Alıntısı Aracı",
            "snippingtool.exe",
            "Ekran Alıntısı Aracı",
        ),
        ("Snipping Tool", "snippingtool.exe", "Snipping Tool"),
        ("Paint", "mspaint.exe", "Paint"),
        ("Event Viewer", "eventvwr.msc", "Event Viewer"),
    ];

    for (name, target, display_name) in tools {
        apps.push(AppItem {
            id: format!("sys_{}", name.to_lowercase().replace(' ', "_")),
            name: display_name.to_string(),
            target_path: target.to_string(),
            arguments: None,
            source_path: format!("system:{}", target),
        });
    }
}

fn scan_dir_for_apps(dir: &PathBuf, apps: &mut Vec<AppItem>, depth: usize) {
    if depth > 6 {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_dir_for_apps(&path, apps, depth + 1);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if ext_lower == "lnk" || ext_lower == "url" || (ext_lower == "exe" && depth <= 1) {
                    let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if !file_stem.is_empty() && !is_junk_app(file_stem) {
                        apps.push(AppItem {
                            id: path.to_string_lossy().to_string(),
                            name: file_stem.to_string(),
                            target_path: path.to_string_lossy().to_string(),
                            arguments: None,
                            source_path: path.to_string_lossy().to_string(),
                        });
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_index_serialization() {
        let items = vec![
            AppItem {
                id: "test1".to_string(),
                name: "Calculator".to_string(),
                target_path: "calc.exe".to_string(),
                arguments: None,
                source_path: "calc.lnk".to_string(),
            },
            AppItem {
                id: "test2".to_string(),
                name: "Visual Studio Code".to_string(),
                target_path: "code.exe".to_string(),
                arguments: Some("--new-window".to_string()),
                source_path: "code.lnk".to_string(),
            },
        ];

        let index = AppIndex {
            items: items.clone(),
        };
        let serialized = serde_json::to_string(&index).unwrap();
        let deserialized: AppIndex = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.items.len(), 2);
        assert_eq!(deserialized.items[0].name, "Calculator");
        assert_eq!(deserialized.items[1].name, "Visual Studio Code");
    }

    #[test]
    fn test_app_discovery_contains_system_tools_and_apps() {
        let apps = discover_applications();
        assert!(!apps.is_empty());

        let names: Vec<String> = apps.iter().map(|a| a.name.to_lowercase()).collect();
        assert!(
            names
                .iter()
                .any(|n| n.contains("control panel") || n.contains("denetim masası"))
        );
        assert!(
            names
                .iter()
                .any(|n| n.contains("settings") || n.contains("ayarlar"))
        );
        assert!(
            names
                .iter()
                .any(|n| n.contains("task manager") || n.contains("görev yöneticisi"))
        );
        assert!(
            names
                .iter()
                .any(|n| n.contains("notepad") || n.contains("not defteri"))
        );
    }

    #[test]
    fn test_is_junk_app() {
        assert!(is_junk_app("unins000"));
        assert!(is_junk_app("Uninstall Antigravity"));
        assert!(is_junk_app(
            "VLC media player - reset preferences and cache files"
        ));
        assert!(is_junk_app("Unity Release Notes"));
        assert!(!is_junk_app("Counter-Strike 2"));
        assert!(!is_junk_app("VALORANT"));
        assert!(!is_junk_app("Watch Dogs 2"));
        assert!(!is_junk_app("PC Manager"));
    }

    #[test]
    fn test_game_discovery_finds_installed_games() {
        let index = AppIndex::rebuild();
        let names: Vec<String> = index.items.iter().map(|a| a.name.to_lowercase()).collect();
        println!("Total discovered items: {}", index.items.len());

        // Check if Steam games on this PC are found
        let has_cs = names.iter().any(|n| n.contains("counter-strike"));
        let has_ets = names.iter().any(|n| n.contains("euro truck"));
        let has_val = names.iter().any(|n| n.contains("valorant"));

        println!(
            "Found CS: {}, Found ETS: {}, Found Valorant: {}",
            has_cs, has_ets, has_val
        );
        assert!(
            has_cs || has_ets || has_val,
            "At least one installed game must be found"
        );
    }
}
