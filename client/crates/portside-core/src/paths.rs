//! Where Portside keeps its files. Every OS path in the client is resolved here
//! (client/CLAUDE.md, "Paths").

use std::path::{Path, PathBuf};

use directories::{BaseDirs, ProjectDirs, UserDirs};

use crate::error::CoreError;
use crate::types::Os;

/// Per-user directories for Portside's own data.
#[derive(Debug, Clone)]
pub struct AppPaths {
    /// Database, settings and caches: `{app_data}` in the docs.
    pub data_dir: PathBuf,
    /// Default library root: `{app_data}/library`.
    pub library_dir: PathBuf,
    /// Output of the last run of each game: `{app_data}/logs`.
    pub logs_dir: PathBuf,
}

impl AppPaths {
    /// Standard per-user locations (e.g. `~/Library/Application Support/gg.portside.Portside`).
    pub fn discover() -> Result<AppPaths, CoreError> {
        let dirs = ProjectDirs::from("gg", "portside", "Portside").ok_or(CoreError::NoHomeDir)?;
        Ok(AppPaths::under(dirs.data_dir().to_path_buf()))
    }

    /// All paths under one root; used by tests and portable installs.
    pub fn under(data_dir: PathBuf) -> AppPaths {
        AppPaths {
            library_dir: data_dir.join("library"),
            logs_dir: data_dir.join("logs"),
            data_dir,
        }
    }
}

/// Resolves a manifest path such as `{app_support}/soh/Save/` on this computer
/// (docs/specs/manifest-format.md, "Path placeholders"). `install` is the install folder
/// for `{install}`. None when a placeholder doesn't exist on `os` or isn't supported yet
/// (`{saf:…}`, `{flatpak:…}`), or the path has an unknown placeholder.
pub fn resolve_placeholders(template: &str, os: Os, install: Option<&Path>) -> Option<PathBuf> {
    let base = BaseDirs::new()?;
    let user = UserDirs::new();
    let (name, rest) = match template.strip_prefix('{') {
        Some(t) => {
            let end = t.find('}')?;
            (Some(&t[..end]), &t[end + 1..])
        }
        None => (None, template),
    };
    let root: PathBuf = match (name, os) {
        (None, _) => return Some(PathBuf::from(template)).filter(|p| p.is_absolute()),
        (Some("install"), _) => install?.to_path_buf(),
        (Some("home"), Os::Windows | Os::Macos | Os::Linux) => base.home_dir().to_path_buf(),
        (Some("appdata"), Os::Windows) => base.config_dir().to_path_buf(),
        (Some("localappdata"), Os::Windows) => base.data_local_dir().to_path_buf(),
        (Some("documents"), Os::Windows | Os::Macos | Os::Linux) => {
            user.as_ref()?.document_dir()?.to_path_buf()
        }
        (Some("app_support"), Os::Macos) => base.data_dir().to_path_buf(),
        (Some("xdg_data"), Os::Linux) => base.data_dir().to_path_buf(),
        (Some("xdg_config"), Os::Linux) => base.config_dir().to_path_buf(),
        _ => return None,
    };
    if rest.contains('{') {
        return None;
    }
    let mut path = root;
    for part in rest.split(['/', '\\']).filter(|p| !p.is_empty()) {
        if part == ".." {
            return None;
        }
        path.push(part);
    }
    Some(path)
}

/// Steam folders that exist on this computer (the ones holding `userdata/`), most likely
/// first: the usual install, then Flatpak and Snap on Linux. Steam installed elsewhere on
/// Windows isn't found yet [TBD: read SteamPath from the registry].
pub fn steam_dirs() -> Vec<PathBuf> {
    let Some(base) = BaseDirs::new() else {
        return Vec::new();
    };
    let home = base.home_dir();
    let candidates: Vec<PathBuf> = if cfg!(target_os = "linux") {
        vec![
            home.join(".local/share/Steam"),
            home.join(".steam/steam"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
            home.join("snap/steam/common/.local/share/Steam"),
        ]
    } else if cfg!(target_os = "macos") {
        vec![home.join("Library/Application Support/Steam")]
    } else if cfg!(target_os = "windows") {
        ["ProgramFiles(x86)", "ProgramFiles"]
            .iter()
            .filter_map(std::env::var_os)
            .map(|dir| PathBuf::from(dir).join("Steam"))
            .collect()
    } else {
        Vec::new()
    };
    let mut found: Vec<PathBuf> = Vec::new();
    for dir in candidates {
        if !dir.join("userdata").is_dir() {
            continue;
        }
        // ~/.steam/steam is usually a link to ~/.local/share/Steam.
        let real = dir.canonicalize().unwrap_or(dir);
        if !found.contains(&real) {
            found.push(real);
        }
    }
    found
}

/// True on a Steam Deck (Valve's own hardware). Installing to the SD card is offered only
/// there (docs/platforms.md, "Steam Deck"). `PORTSIDE_SD_CARD` pretends to be one.
pub fn is_steam_deck() -> bool {
    if std::env::var_os("PORTSIDE_SD_CARD").is_some() {
        return true;
    }
    cfg!(target_os = "linux")
        && std::fs::read_to_string("/sys/devices/virtual/dmi/id/board_vendor")
            .is_ok_and(|v| v.trim() == "Valve")
}

/// The Steam Deck's SD card, when one is in: where SteamOS mounted it. Elsewhere None.
/// `PORTSIDE_SD_CARD=<folder>` stands in for a card when testing on another computer.
pub fn sd_card() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("PORTSIDE_SD_CARD") {
        return Some(PathBuf::from(dir)).filter(|p| p.is_dir());
    }
    if !is_steam_deck() {
        return None;
    }
    let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
    sd_card_in(&mounts).filter(|p| p.is_dir())
}

/// Where games go on the SD card: a `Portside` folder at its top, beside Steam's own.
pub fn sd_card_library(card: &Path) -> PathBuf {
    card.join("Portside")
}

/// True when `path` is on a card or drive that can be taken out (SteamOS mounts those under
/// `/run/media`), so missing files may only mean it isn't in.
pub fn on_removable_storage(path: &Path) -> bool {
    cfg!(target_os = "linux") && path.starts_with("/run/media")
}

/// The SD card's mount point in a `/proc/mounts` listing: the reader's device (`mmcblk`)
/// mounted under `/run/media` (`/run/media/deck/<label>`, or `/run/media/mmcblk0p1` before
/// SteamOS 3.5).
fn sd_card_in(mounts: &str) -> Option<PathBuf> {
    mounts.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let device = fields.next()?;
        let dir = unescape_mount(fields.next()?);
        (device.starts_with("/dev/mmcblk") && dir.starts_with("/run/media/"))
            .then(|| PathBuf::from(dir))
    })
}

/// `/proc/mounts` writes spaces, tabs, newlines and backslashes as `\040`-style octal.
fn unescape_mount(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let code = bytes
            .get(i + 1..i + 4)
            .filter(|_| bytes[i] == b'\\')
            .and_then(|d| std::str::from_utf8(d).ok())
            .and_then(|d| u8::from_str_radix(d, 8).ok());
        match code {
            Some(c) => {
                out.push(c);
                i += 4;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A folder kept in sync by a cloud app, offered when choosing where cloud saves go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncedFolder {
    /// e.g. "iCloud Drive".
    pub label: String,
    pub path: PathBuf,
}

/// Synced folders that exist on this computer: iCloud Drive, Dropbox, OneDrive and Google
/// Drive for desktop in their usual places.
pub fn synced_folders() -> Vec<SyncedFolder> {
    let Some(base) = BaseDirs::new() else {
        return Vec::new();
    };
    let home = base.home_dir();
    let mut found = Vec::new();
    let mut add = |label: &str, path: PathBuf| {
        if path.is_dir() && !found.iter().any(|f: &SyncedFolder| f.path == path) {
            found.push(SyncedFolder {
                label: label.to_owned(),
                path,
            });
        }
    };
    add(
        "iCloud Drive",
        home.join("Library/Mobile Documents/com~apple~CloudDocs"),
    );
    add("iCloud Drive", home.join("iCloudDrive"));
    add("Dropbox", home.join("Dropbox"));
    add("OneDrive", home.join("OneDrive"));
    add("Google Drive", home.join("Google Drive"));
    // macOS File Provider folders: ~/Library/CloudStorage/{Dropbox,OneDrive-…,GoogleDrive-…}.
    if let Ok(entries) = std::fs::read_dir(home.join("Library/CloudStorage")) {
        let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for path in entries {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.starts_with("GoogleDrive") {
                // Files go in "My Drive" inside the account folder.
                let my_drive = path.join("My Drive");
                add(
                    "Google Drive",
                    if my_drive.is_dir() { my_drive } else { path },
                );
            } else if name.starts_with("OneDrive") {
                add("OneDrive", path);
            } else if name.starts_with("Dropbox") {
                add("Dropbox", path);
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_sd_card_mount() {
        let mounts = "/dev/nvme0n1p8 /home ext4 rw 0 0\n\
            /dev/sda1 /run/media/deck/USB ext4 rw 0 0\n\
            /dev/mmcblk0p1 /run/media/deck/My\\040Card ext4 rw 0 0\n";
        assert_eq!(
            sd_card_in(mounts),
            Some(PathBuf::from("/run/media/deck/My Card"))
        );
        assert_eq!(
            sd_card_in("/dev/mmcblk0p1 /run/media/mmcblk0p1 ext4 rw 0 0"),
            Some(PathBuf::from("/run/media/mmcblk0p1"))
        );
        assert_eq!(sd_card_in("/dev/mmcblk0p2 /boot vfat rw 0 0"), None);
        assert_eq!(unescape_mount("a\\134b\\011"), "a\\b\t");
    }

    #[test]
    fn library_lives_under_data_dir() {
        let p = AppPaths::under(PathBuf::from("/tmp/portside-test"));
        assert_eq!(p.library_dir, PathBuf::from("/tmp/portside-test/library"));
    }

    #[test]
    fn placeholders_resolve_per_os() {
        let install = PathBuf::from("/games/x/v1");
        assert_eq!(
            resolve_placeholders("{install}/Save/", Os::Linux, Some(&install)),
            Some(install.join("Save"))
        );
        assert_eq!(
            resolve_placeholders("{install}/Save", Os::Linux, None),
            None
        );
        assert_eq!(
            resolve_placeholders("{app_support}/x", Os::Linux, None),
            None
        );
        assert_eq!(
            resolve_placeholders("{saf:shared}/x", Os::Android, None),
            None
        );
        assert_eq!(
            resolve_placeholders("{install}/../x", Os::Linux, Some(&install)),
            None
        );
        assert_eq!(resolve_placeholders("relative/x", Os::Linux, None), None);
        if let Some(home) = BaseDirs::new().map(|b| b.home_dir().to_path_buf()) {
            assert_eq!(
                resolve_placeholders("{home}/.game/saves", Os::Linux, None),
                Some(home.join(".game").join("saves"))
            );
        }
    }
}
