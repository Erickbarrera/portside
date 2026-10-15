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
