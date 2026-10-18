//! Proton, Steam's way of running Windows games on Linux (client/docs/platforms.md,
//! "Proton"). Portside uses a Proton the player installed through Steam to run ports that
//! only have a Windows build.

use std::path::{Path, PathBuf};

/// A Proton install: the folder holding its `proton` script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proton {
    /// e.g. "Proton 9.0".
    pub name: String,
    pub dir: PathBuf,
    /// The Steam folder it belongs to (Proton wants it as `STEAM_COMPAT_CLIENT_INSTALL_PATH`).
    pub steam_dir: PathBuf,
}

impl Proton {
    /// The script that runs a Windows program: `proton run <exe>`.
    pub fn script(&self) -> PathBuf {
        self.dir.join("proton")
    }
}

/// The best Proton on this computer: the newest numbered release from Valve, else Proton
/// Experimental, else a community build (`compatibilitytools.d`). `PORTSIDE_PROTON=<folder>`
/// picks one. None on other systems or when Steam has none installed.
pub fn find_proton() -> Option<Proton> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let steam_dirs = portside_core::paths::steam_dirs();
    if let Some(dir) = std::env::var_os("PORTSIDE_PROTON").map(PathBuf::from) {
        let steam_dir = steam_dirs.first().cloned().unwrap_or_default();
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        return dir.join("proton").is_file().then_some(Proton {
            name,
            dir,
            steam_dir,
        });
    }
    let mut found = Vec::new();
    for steam_dir in &steam_dirs {
        found.extend(protons_in(steam_dir));
    }
    best(found)
}

/// Every Proton under one Steam folder: in each of its libraries' `steamapps/common`, and
/// in `compatibilitytools.d`.
fn protons_in(steam_dir: &Path) -> Vec<Proton> {
    let mut folders = vec![steam_dir.join("compatibilitytools.d")];
    let libraries = crate::apps::libraries(steam_dir);
    folders.extend(libraries.iter().map(|l| l.join("steamapps/common")));
    let mut found = Vec::new();
    for folder in folders {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for dir in entries.flatten().map(|e| e.path()) {
            let name = dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.contains("Proton") && dir.join("proton").is_file() {
                found.push(Proton {
                    name,
                    dir,
                    steam_dir: steam_dir.to_path_buf(),
                });
            }
        }
    }
    found
}

/// How much a Proton is preferred, higher first: Valve's numbered releases by version, then
/// Experimental, then others (GE-Proton and the like) by version.
fn rank(p: &Proton) -> (u8, Vec<u32>) {
    let version: Vec<u32> = p
        .name
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    let valve = p.name.starts_with("Proton ") || p.name.starts_with("Proton -");
    let kind = if valve && p.name.contains("Experimental") {
        2
    } else if valve && !version.is_empty() && !p.name.contains("Beta") {
        3
    } else if valve {
        1
    } else {
        0
    };
    (kind, version)
}

fn best(found: Vec<Proton>) -> Option<Proton> {
    found.into_iter().max_by(|a, b| rank(a).cmp(&rank(b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proton(name: &str) -> Proton {
        Proton {
            name: name.to_owned(),
            dir: PathBuf::from(format!("/steam/steamapps/common/{name}")),
            steam_dir: PathBuf::from("/steam"),
        }
    }

    #[test]
    fn prefers_the_newest_valve_release() {
        let pick = |names: &[&str]| best(names.iter().map(|n| proton(n)).collect()).map(|p| p.name);
        assert_eq!(
            pick(&[
                "Proton 8.0",
                "Proton - Experimental",
                "Proton 9.0",
                "GE-Proton9-20"
            ]),
            Some("Proton 9.0".into())
        );
        assert_eq!(
            pick(&["GE-Proton9-20", "Proton - Experimental"]),
            Some("Proton - Experimental".into())
        );
        assert_eq!(
            pick(&["GE-Proton8-1", "GE-Proton9-20"]),
            Some("GE-Proton9-20".into())
        );
        assert_eq!(pick(&[]), None);
    }

    #[test]
    fn finds_protons_in_a_steam_folder() -> Result<(), Box<dyn std::error::Error>> {
        let steam = tempfile::tempdir()?;
        let card = tempfile::tempdir()?;
        let make = |dir: &Path| -> std::io::Result<()> {
            std::fs::create_dir_all(dir)?;
            std::fs::write(dir.join("proton"), "#!/usr/bin/env python3\n")
        };
        make(&steam.path().join("steamapps/common/Proton 8.0"))?;
        make(&card.path().join("steamapps/common/Proton 9.0"))?;
        make(&steam.path().join("compatibilitytools.d/GE-Proton9-20"))?;
        // A game, not a Proton.
        std::fs::create_dir_all(steam.path().join("steamapps/common/Example Game"))?;
        std::fs::write(
            steam.path().join("steamapps/libraryfolders.vdf"),
            format!("\"0\"\n{{\n\"path\" \"{}\"\n}}\n", card.path().display()),
        )?;
        let mut names: Vec<String> = protons_in(steam.path())
            .into_iter()
            .map(|p| p.name)
            .collect();
        names.sort();
        assert_eq!(names, ["GE-Proton9-20", "Proton 8.0", "Proton 9.0"]);
        Ok(())
    }
}
