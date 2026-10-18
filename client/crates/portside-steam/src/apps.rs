//! Games Steam installed: each library in `steamapps/libraryfolders.vdf`, and the
//! `appmanifest_<id>.acf` Steam keeps there for each game (docs/specs/manifest-format.md,
//! "game_files").

use std::path::{Path, PathBuf};

/// Every library of one Steam folder: the folder itself, then the others it lists.
pub fn libraries(steam_dir: &Path) -> Vec<PathBuf> {
    let mut found = vec![steam_dir.to_path_buf()];
    if let Ok(text) = std::fs::read_to_string(steam_dir.join("steamapps/libraryfolders.vdf")) {
        for lib in library_paths(&text) {
            if !found.contains(&lib) {
                found.push(lib);
            }
        }
    }
    found
}

/// The `"path"` of each library in Steam's text `libraryfolders.vdf`.
pub(crate) fn library_paths(text: &str) -> Vec<PathBuf> {
    pairs(text)
        .filter(|(k, _)| k.eq_ignore_ascii_case("path"))
        .map(|(_, v)| PathBuf::from(v.replace("\\\\", "\\")))
        .collect()
}

/// The folder of Steam app `appid` when one of these Steam folders has it fully installed.
pub fn installed_app(steam_dirs: &[PathBuf], appid: u32) -> Option<PathBuf> {
    steam_dirs
        .iter()
        .flat_map(|dir| libraries(dir))
        .find_map(|lib| app_in(&lib, appid))
}

fn app_in(library: &Path, appid: u32) -> Option<PathBuf> {
    let apps = library.join("steamapps");
    let text = std::fs::read_to_string(apps.join(format!("appmanifest_{appid}.acf"))).ok()?;
    let mut installdir = None;
    let mut flags = None;
    for (k, v) in pairs(&text) {
        if installdir.is_none() && k.eq_ignore_ascii_case("installdir") {
            installdir = Some(v);
        } else if flags.is_none() && k.eq_ignore_ascii_case("StateFlags") {
            flags = v.parse::<u32>().ok();
        }
    }
    // Bit 4 is "fully installed"; a game still downloading or never finished is left out.
    let installdir = installdir.filter(|d| !d.is_empty() && !d.contains(['/', '\\']))?;
    if flags.is_some_and(|f| f & 4 == 0) {
        return None;
    }
    let dir = apps.join("common").join(installdir);
    dir.is_dir().then_some(dir)
}

/// `"key" "value"` pairs, one per line, in a text KeyValues file.
fn pairs(text: &str) -> impl Iterator<Item = (&str, &str)> {
    text.lines().filter_map(|line| {
        let mut quoted = line.split('"').skip(1).step_by(2);
        Some((quoted.next()?, quoted.next()?))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_library_folders() {
        let text = r#""libraryfolders"
{
	"0"
	{
		"path"		"/home/deck/.local/share/Steam"
		"label"		""
	}
	"1"
	{
		"path"		"/run/media/deck/Card"
	}
}"#;
        assert_eq!(
            library_paths(text),
            vec![
                PathBuf::from("/home/deck/.local/share/Steam"),
                PathBuf::from("/run/media/deck/Card")
            ]
        );
        assert_eq!(
            library_paths(r#""path"		"D:\\SteamLibrary""#),
            vec![PathBuf::from("D:\\SteamLibrary")]
        );
    }

    fn manifest(lib: &Path, appid: u32, dir: &str, flags: u32) -> std::io::Result<()> {
        std::fs::create_dir_all(lib.join("steamapps/common").join(dir))?;
        std::fs::write(
            lib.join(format!("steamapps/appmanifest_{appid}.acf")),
            format!(
                "\"AppState\"\n{{\n\t\"appid\"\t\t\"{appid}\"\n\t\"name\"\t\t\"Example\"\n\t\"StateFlags\"\t\t\"{flags}\"\n\t\"installdir\"\t\t\"{dir}\"\n}}\n"
            ),
        )
    }

    #[test]
    fn finds_installed_games_in_every_library() -> Result<(), Box<dyn std::error::Error>> {
        let steam = tempfile::tempdir()?;
        let card = tempfile::tempdir()?;
        std::fs::create_dir_all(steam.path().join("steamapps"))?;
        std::fs::write(
            steam.path().join("steamapps/libraryfolders.vdf"),
            format!("\"0\"\n{{\n\"path\" \"{}\"\n}}\n", card.path().display()),
        )?;
        manifest(steam.path(), 10, "Example One", 4)?;
        manifest(card.path(), 20, "Example Two", 6)?;
        // Still downloading.
        manifest(card.path(), 30, "Example Three", 1026)?;
        let dirs = [steam.path().to_path_buf()];
        assert_eq!(
            installed_app(&dirs, 10),
            Some(steam.path().join("steamapps/common/Example One"))
        );
        assert_eq!(
            installed_app(&dirs, 20),
            Some(card.path().join("steamapps/common/Example Two"))
        );
        assert_eq!(installed_app(&dirs, 30), None);
        assert_eq!(installed_app(&dirs, 40), None);
        Ok(())
    }
}
