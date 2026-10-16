//! Finds Proton for Windows-only ports (`proton.rs`), and adds installed ports to Steam as
//! non-Steam games (client/docs/platforms.md, "Add to
//! Steam"), on every OS Steam runs on. Each shortcut starts Portside with `--play <port>`,
//! so updates (which move the game to a new folder) never break it and cloud saves still
//! sync. Steam reads `shortcuts.vdf` when it starts and may write over it while running, so
//! the file is only changed while Steam is closed.

mod art;
mod proton;
pub mod vdf;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sysinfo::{ProcessRefreshKind, RefreshKind, System};
use vdf::{Map, Value};

pub use art::write_art;
pub use proton::{Proton, find_proton};

#[derive(Debug, thiserror::Error)]
pub enum SteamError {
    #[error("Steam isn't installed, or hasn't been signed in to on this computer")]
    NotFound,
    #[error("Steam's list of non-Steam games is damaged (at byte {0})")]
    BadShortcuts(usize),
    #[error("couldn't read or write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the artwork isn't an image Portside can read: {0}")]
    Image(#[from] image::ImageError),
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> SteamError + '_ {
    move |source| SteamError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// One Steam account's folder on this computer: `<steam>/userdata/<account id>`.
#[derive(Debug, Clone)]
pub struct Steam {
    user_dir: PathBuf,
}

/// What Portside puts in Steam for a game.
#[derive(Debug, Clone)]
pub struct Shortcut {
    /// Shown in Steam's library.
    pub name: String,
    /// The program Steam starts (Portside itself).
    pub exe: PathBuf,
    pub start_dir: PathBuf,
    /// How the shortcut is recognised again later, e.g. `--play example-rpg`.
    pub launch_options: String,
}

impl Steam {
    /// The Steam account last used on this computer, if Steam is installed.
    pub fn find() -> Result<Steam, SteamError> {
        portside_core::paths::steam_dirs()
            .iter()
            .find_map(|dir| Steam::latest_user(dir))
            .ok_or(SteamError::NotFound)
    }

    /// The account under `steam_dir/userdata` whose settings changed last. Folder `0` is
    /// Steam's signed-out placeholder.
    fn latest_user(steam_dir: &Path) -> Option<Steam> {
        let entries = std::fs::read_dir(steam_dir.join("userdata")).ok()?;
        entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n != "0" && n.chars().all(|c| c.is_ascii_digit()))
            })
            .max_by_key(|p| {
                std::fs::metadata(p.join("config/localconfig.vdf"))
                    .or_else(|_| std::fs::metadata(p))
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH)
            })
            .map(|user_dir| Steam { user_dir })
    }

    /// An account folder given directly (tests).
    pub fn at(user_dir: PathBuf) -> Steam {
        Steam { user_dir }
    }

    fn shortcuts_file(&self) -> PathBuf {
        self.user_dir.join("config/shortcuts.vdf")
    }

    /// Where Steam looks for custom artwork.
    pub fn grid_dir(&self) -> PathBuf {
        self.user_dir.join("config/grid")
    }

    fn read(&self) -> Result<Map, SteamError> {
        let path = self.shortcuts_file();
        match std::fs::read(&path) {
            Ok(bytes) => vdf::parse(&bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::default()),
            Err(e) => Err(io(&path)(e)),
        }
    }

    /// The Steam app id of the shortcut with these launch options, if there is one.
    pub fn find_shortcut(&self, launch_options: &str) -> Result<Option<u32>, SteamError> {
        let file = self.read()?;
        let Some(Value::Map(list)) = file.get("shortcuts") else {
            return Ok(None);
        };
        Ok(list.0.iter().find_map(|(_, v)| match v {
            Value::Map(s) if s.str("LaunchOptions") == Some(launch_options) => {
                Some(s.int("appid").unwrap_or_else(|| {
                    shortcut_id(
                        s.str("Exe").unwrap_or_default(),
                        s.str("AppName").unwrap_or_default(),
                    )
                }))
            }
            _ => None,
        }))
    }

    /// Adds the shortcut, or updates the one with the same launch options. Returns its app
    /// id (the name of its artwork files). Only call while Steam is closed.
    pub fn add(&self, shortcut: &Shortcut) -> Result<u32, SteamError> {
        let path = self.shortcuts_file();
        let mut file = self.read()?;
        if file.map_mut("shortcuts").is_none() {
            file.set("shortcuts", Value::Map(Map::default()));
        }
        let Some(list) = file.map_mut("shortcuts") else {
            return Err(SteamError::BadShortcuts(0));
        };
        let exe = quoted(&shortcut.exe);
        let existing = list.0.iter_mut().find_map(|(_, v)| match v {
            Value::Map(s) if s.str("LaunchOptions") == Some(shortcut.launch_options.as_str()) => {
                Some(s)
            }
            _ => None,
        });
        let appid = match existing {
            Some(entry) => {
                // Keep the id Steam knows it by, so its artwork and play time stay.
                let appid = entry
                    .int("appid")
                    .unwrap_or_else(|| shortcut_id(&exe, &shortcut.name));
                entry.set("appid", Value::Int(appid));
                entry.set("AppName", Value::Str(shortcut.name.clone()));
                entry.set("Exe", Value::Str(exe));
                entry.set("StartDir", Value::Str(quoted(&shortcut.start_dir)));
                appid
            }
            None => {
                let appid = shortcut_id(&exe, &shortcut.name);
                let key = (0..)
                    .map(|n: u32| n.to_string())
                    .find(|k| list.get(k).is_none())
                    .unwrap_or_default();
                list.0
                    .push((key, Value::Map(new_entry(appid, shortcut, exe))));
                appid
            }
        };

        let dir = path.parent().unwrap_or(&self.user_dir);
        std::fs::create_dir_all(dir).map_err(io(dir))?;
        // A one-time copy of the file as it was before Portside first changed it.
        let backup = path.with_extension("vdf.before-portside");
        if path.is_file() && !backup.exists() {
            std::fs::copy(&path, &backup).map_err(io(&backup))?;
        }
        let tmp = path.with_extension("vdf.portside-tmp");
        std::fs::write(&tmp, vdf::write(&file)).map_err(io(&tmp))?;
        std::fs::rename(&tmp, &path).map_err(io(&path))?;
        Ok(appid)
    }
}

fn new_entry(appid: u32, shortcut: &Shortcut, exe: String) -> Map {
    let s = |v: &str| Value::Str(v.to_owned());
    Map(vec![
        ("appid".into(), Value::Int(appid)),
        ("AppName".into(), Value::Str(shortcut.name.clone())),
        ("Exe".into(), Value::Str(exe)),
        ("StartDir".into(), Value::Str(quoted(&shortcut.start_dir))),
        ("icon".into(), s("")),
        ("ShortcutPath".into(), s("")),
        (
            "LaunchOptions".into(),
            Value::Str(shortcut.launch_options.clone()),
        ),
        ("IsHidden".into(), Value::Int(0)),
        ("AllowDesktopConfig".into(), Value::Int(1)),
        ("AllowOverlay".into(), Value::Int(1)),
        ("OpenVR".into(), Value::Int(0)),
        ("Devkit".into(), Value::Int(0)),
        ("DevkitGameID".into(), s("")),
        ("DevkitOverrideAppID".into(), Value::Int(0)),
        ("LastPlayTime".into(), Value::Int(0)),
        ("FlatpakAppID".into(), s("")),
        (
            "tags".into(),
            Value::Map(Map(vec![("0".into(), s("Portside"))])),
        ),
    ])
}

/// Steam's id for a non-Steam game: a CRC-32 of the quoted program and the name, with the
/// top bit set. Its artwork files are named after it.
pub fn shortcut_id(exe: &str, name: &str) -> u32 {
    let mut h = crc32fast::Hasher::new();
    h.update(exe.as_bytes());
    h.update(name.as_bytes());
    h.finalize() | 0x8000_0000
}

fn quoted(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

/// True while the Steam client runs on this computer.
pub fn running() -> bool {
    let sys = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing()),
    );
    let names: &[&str] = if cfg!(target_os = "windows") {
        &["steam.exe"]
    } else if cfg!(target_os = "macos") {
        &["steam_osx"]
    } else {
        &["steam"]
    };
    names
        .iter()
        .any(|n| sys.processes_by_exact_name(n.as_ref()).next().is_some())
}

#[cfg(test)]
mod tests;
