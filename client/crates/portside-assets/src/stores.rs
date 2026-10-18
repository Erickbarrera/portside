//! Copies of a game that stores installed, from each store's own record
//! (docs/specs/manifest-format.md, "game_files").

use std::path::{Path, PathBuf};

use portside_api::models::GameFiles;
use portside_core::paths::StorePlaces;
use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::files::data_root;
use crate::{Found, Store};

/// Every copy of the game in `places` that holds the files the port needs: Steam first, then
/// GOG, then Epic, in the order the manifest lists the ids. One folder appears once.
pub fn find(spec: &GameFiles, places: &StorePlaces) -> Vec<Found> {
    let Some(stores) = &spec.stores else {
        return Vec::new();
    };
    let mut candidates: Vec<(Store, PathBuf)> = Vec::new();
    for &appid in stores.steam.iter().flatten() {
        if let Some(dir) = portside_steam::apps::installed_app(&places.steam_dirs, appid) {
            candidates.push((Store::Steam, dir));
        }
    }
    for id in stores.gog.iter().flatten() {
        candidates.extend(gog(id, places).into_iter().map(|d| (Store::Gog, d)));
    }
    for name in stores.epic.iter().flatten() {
        candidates.extend(epic(name, places).into_iter().map(|d| (Store::Epic, d)));
    }
    let mut found: Vec<Found> = Vec::new();
    for (store, dir) in candidates {
        if let Some(root) = data_root(&dir, &spec.files)
            && !found.iter().any(|f| f.dir == root)
        {
            found.push(Found { store, dir: root });
        }
    }
    found
}

/// Folders GOG product `id` is installed in: GOG Galaxy's database, Heroic's list, and a
/// `goggame-<id>.info` file in a game folder under GOG's usual folders.
fn gog(id: &str, places: &StorePlaces) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(product) = id.parse::<i64>() {
        for db in &places.gog_galaxy_dbs {
            dirs.extend(galaxy(db, product));
        }
    }
    for heroic in &places.heroic_dirs {
        dirs.extend(heroic_gog(&heroic.join("gog_store/installed.json"), id));
    }
    let marker = format!("goggame-{id}.info");
    for folder in &places.gog_folders {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        dirs.extend(
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.join(&marker).is_file()),
        );
    }
    dirs
}

/// `installationPath` of the product in GOG Galaxy's database, opened read-only.
fn galaxy(db: &Path, product: i64) -> Option<PathBuf> {
    if !db.is_file() {
        return None;
    }
    let conn = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    conn.query_row(
        "SELECT installationPath FROM InstalledBaseProducts WHERE productId = ?1",
        [product],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
    .map(PathBuf::from)
}

/// Heroic's `gog_store/installed.json`: `{ "installed": [{ "appName", "install_path" }] }`.
fn heroic_gog(file: &Path, id: &str) -> Option<PathBuf> {
    let json = read_json(file)?;
    json.get("installed")?
        .as_array()?
        .iter()
        .find(|g| g.get("appName").and_then(|v| v.as_str()) == Some(id))
        .and_then(|g| g.get("install_path")?.as_str())
        .map(PathBuf::from)
}

/// Folders Epic app `name` is installed in: the Epic Games Launcher's `*.item` manifests,
/// and Legendary's (or Heroic's copy of Legendary's) `installed.json`.
fn epic(name: &str, places: &StorePlaces) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for folder in &places.epic_manifest_dirs {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        for item in entries.flatten().map(|e| e.path()).filter(|p| {
            p.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("item"))
        }) {
            let Some(json) = read_json(&item) else {
                continue;
            };
            let incomplete = json
                .get("bIsIncompleteInstall")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if json.get("AppName").and_then(|v| v.as_str()) == Some(name) && !incomplete {
                dirs.extend(
                    json.get("InstallLocation")
                        .and_then(|v| v.as_str())
                        .map(PathBuf::from),
                );
            }
        }
    }
    for file in &places.legendary_files {
        if let Some(path) = read_json(file).and_then(|j| {
            j.get(name)?
                .get("install_path")?
                .as_str()
                .map(PathBuf::from)
        }) {
            dirs.push(path);
        }
    }
    dirs
}

fn read_json(file: &Path) -> Option<serde_json::Value> {
    serde_json::from_slice(&std::fs::read(file).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use portside_api::models::GameStores;

    fn spec(steam: &[u32], gog: &[&str], epic: &[&str]) -> GameFiles {
        let list = |v: &[&str]| Some(v.iter().map(|s| s.to_string()).collect());
        GameFiles {
            label: "Example Game".into(),
            stores: Some(GameStores {
                steam: Some(steam.to_vec()),
                gog: list(gog),
                epic: list(epic),
            }),
            files: vec!["data.pak".into()],
            dest: None,
        }
    }

    fn game(dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("data.pak"), b"synthetic")
    }

    #[test]
    fn finds_copies_from_every_store() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let r = root.path();

        // Steam.
        let steam = r.join("steam");
        std::fs::create_dir_all(steam.join("steamapps"))?;
        std::fs::write(
            steam.join("steamapps/appmanifest_10.acf"),
            "\"AppState\"\n{\n\"StateFlags\" \"4\"\n\"installdir\" \"Example Game\"\n}\n",
        )?;
        game(&steam.join("steamapps/common/Example Game"))?;

        // GOG Galaxy.
        let galaxy_game = r.join("galaxy/Example Game");
        game(&galaxy_game)?;
        let db = r.join("galaxy-2.0.db");
        {
            let conn = Connection::open(&db)?;
            conn.execute_batch(
                "CREATE TABLE InstalledBaseProducts (productId INTEGER, installationPath TEXT);",
            )?;
            conn.execute(
                "INSERT INTO InstalledBaseProducts VALUES (20, ?1)",
                [galaxy_game.to_string_lossy()],
            )?;
        }

        // Heroic (GOG), with the files one folder down.
        let heroic = r.join("heroic");
        let heroic_game = r.join("Games/Heroic/Example Game");
        game(&heroic_game.join("app"))?;
        std::fs::create_dir_all(heroic.join("gog_store"))?;
        std::fs::write(
            heroic.join("gog_store/installed.json"),
            serde_json::json!({ "installed": [
                { "appName": "21", "install_path": heroic_game, "platform": "windows" }
            ]})
            .to_string(),
        )?;

        // A plain GOG installer.
        let gog_folder = r.join("GOG Games");
        game(&gog_folder.join("Example"))?;
        std::fs::write(gog_folder.join("Example/goggame-22.info"), "{}")?;

        // Epic Games Launcher and Legendary.
        let manifests = r.join("Manifests");
        let epic_game = r.join("Epic/ExampleGame");
        game(&epic_game)?;
        std::fs::create_dir_all(&manifests)?;
        std::fs::write(
            manifests.join("ABC.item"),
            serde_json::json!({ "AppName": "Example", "InstallLocation": epic_game }).to_string(),
        )?;
        let legendary = r.join("legendary.json");
        let legendary_game = r.join("legendary/Example");
        game(&legendary_game)?;
        std::fs::write(
            &legendary,
            serde_json::json!({ "Other": { "install_path": legendary_game } }).to_string(),
        )?;

        let places = StorePlaces {
            steam_dirs: vec![steam.clone()],
            gog_galaxy_dbs: vec![db],
            gog_folders: vec![gog_folder.clone()],
            heroic_dirs: vec![heroic],
            epic_manifest_dirs: vec![manifests],
            legendary_files: vec![legendary],
        };
        let found = find(
            &spec(&[10, 11], &["20", "21", "22"], &["Example", "Other"]),
            &places,
        );
        let got: Vec<(Store, PathBuf)> = found.into_iter().map(|f| (f.store, f.dir)).collect();
        assert_eq!(
            got,
            vec![
                (Store::Steam, steam.join("steamapps/common/Example Game")),
                (Store::Gog, galaxy_game),
                (Store::Gog, heroic_game.join("app")),
                (Store::Gog, gog_folder.join("Example")),
                (Store::Epic, epic_game),
                (Store::Epic, legendary_game),
            ]
        );

        // Ids the manifest doesn't list aren't looked at; a copy without the files is skipped.
        assert!(find(&spec(&[], &["99"], &[]), &places).is_empty());
        std::fs::remove_file(steam.join("steamapps/common/Example Game/data.pak"))?;
        assert!(find(&spec(&[10], &[], &[]), &places).is_empty());
        Ok(())
    }
}
