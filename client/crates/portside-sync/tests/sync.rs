//! Two devices sharing one vault folder, as two computers sharing a synced cloud folder.

use std::path::{Path, PathBuf};

use portside_core::Os;
use portside_sync::{
    Device, FolderStorage, LocalState, SaveRules, SyncError, SyncState, Syncer, Vault,
};

const PORT: &str = "example-port";

struct Pc {
    vault: Vault,
    state: LocalState,
    device: Device,
    rules: SaveRules,
    trash: PathBuf,
}

impl Pc {
    fn new(dir: &Path, name: &str, vault: Vault) -> Pc {
        let state = LocalState::open(&dir.join(name).join("saves.db")).expect("db");
        Pc {
            vault,
            device: Device {
                id: state.device_id().expect("id"),
                name: name.to_owned(),
                os: Os::Linux,
                app_version: "0.1.0".to_owned(),
            },
            state,
            rules: SaveRules {
                root: dir.join(name).join("game-saves"),
                include: vec!["**/*.sav".to_owned()],
                exclude: vec![],
            },
            trash: dir.join(name).join("trash"),
        }
    }

    fn syncer(&self) -> Syncer<'_> {
        Syncer {
            vault: &self.vault,
            state: &self.state,
            device: &self.device,
            trash_root: &self.trash,
        }
    }

    fn sync(&self) -> portside_sync::SyncOutcome {
        self.syncer()
            .sync(PORT, &self.rules, Some("v1"))
            .expect("sync")
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.rules.root.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
        std::fs::write(path, text).expect("write");
    }

    fn read(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.rules.root.join(rel)).ok()
    }
}

fn two_pcs(dir: &Path) -> (Pc, Pc) {
    let cloud = dir.join("cloud");
    std::fs::create_dir_all(&cloud).expect("cloud");
    let a = Vault::open_or_create(Box::new(FolderStorage::new(&cloud))).expect("create");
    let b = Vault::open_or_create(Box::new(FolderStorage::new(&cloud))).expect("open");
    assert_eq!(a.id(), b.id());
    (Pc::new(dir, "a", a), Pc::new(dir, "b", b))
}

#[test]
fn saves_follow_the_player_between_devices() {
    let dir = tempfile::tempdir().expect("tmp");
    let (a, b) = two_pcs(dir.path());

    a.write("slot1.sav", "level 1");
    a.write("notes.txt", "not a save");
    assert_eq!(a.sync().state, SyncState::Uploaded);
    assert_eq!(a.sync().state, SyncState::UpToDate);

    let out = b.sync();
    assert_eq!(out.state, SyncState::Restored);
    assert_eq!(b.read("slot1.sav").as_deref(), Some("level 1"));
    assert_eq!(b.read("notes.txt"), None);
    assert_eq!(out.current.expect("current").device_name, "a");

    b.write("slot1.sav", "level 2");
    b.write("sub/slot2.sav", "new slot");
    assert_eq!(b.sync().state, SyncState::Uploaded);
    assert_eq!(a.sync().state, SyncState::Restored);
    assert_eq!(a.read("slot1.sav").as_deref(), Some("level 2"));
    assert_eq!(a.read("sub/slot2.sav").as_deref(), Some("new slot"));
    // A's old save was copied aside before it was replaced.
    let trash: Vec<_> = walk(&a.trash);
    assert!(
        trash
            .iter()
            .any(|p| std::fs::read_to_string(p).ok().as_deref() == Some("level 1"))
    );

    std::fs::remove_file(a.rules.root.join("sub/slot2.sav")).expect("rm");
    assert_eq!(a.sync().state, SyncState::Uploaded);
    assert_eq!(b.sync().state, SyncState::Restored);
    assert_eq!(b.read("sub/slot2.sav"), None);

    let devices = a.vault.devices().expect("devices");
    assert_eq!(devices.len(), 2);
}

#[test]
fn different_saves_on_two_devices_are_a_conflict_until_one_is_kept() {
    let dir = tempfile::tempdir().expect("tmp");
    let (a, b) = two_pcs(dir.path());
    a.write("slot1.sav", "start");
    a.sync();
    b.sync();

    // Both play offline.
    a.write("slot1.sav", "a went left");
    b.write("slot1.sav", "b went right");
    assert_eq!(a.sync().state, SyncState::Uploaded);
    let out = b.sync();
    assert_eq!(out.state, SyncState::Conflict);
    assert_eq!(out.conflict.len(), 2);
    // Nothing was overwritten.
    assert_eq!(b.read("slot1.sav").as_deref(), Some("b went right"));
    assert_eq!(a.sync().state, SyncState::Conflict);

    let a_side = out
        .conflict
        .iter()
        .find(|s| !s.this_device)
        .expect("a's side");
    assert_eq!(a_side.device_name, "a");
    let kept = b
        .syncer()
        .keep(PORT, &b.rules, &a_side.id, Some("v1"))
        .expect("keep");
    assert_eq!(kept.state, SyncState::Restored);
    assert_eq!(b.read("slot1.sav").as_deref(), Some("a went left"));

    assert_eq!(a.sync().state, SyncState::UpToDate);
    assert_eq!(b.sync().state, SyncState::UpToDate);
    assert!(matches!(
        b.syncer().keep(PORT, &b.rules, "nope", None),
        Err(SyncError::UnknownSnapshot)
    ));
}

#[test]
fn a_new_device_with_its_own_saves_asks_first() {
    let dir = tempfile::tempdir().expect("tmp");
    let (a, b) = two_pcs(dir.path());
    a.write("slot1.sav", "from a");
    a.sync();
    b.write("slot1.sav", "from b, before sync was on");
    assert_eq!(b.sync().state, SyncState::Conflict);
}

#[test]
fn one_vault_per_folder() {
    let dir = tempfile::tempdir().expect("tmp");
    let (a, _) = two_pcs(dir.path());
    let cloud = dir.path().join("cloud");
    assert!(matches!(
        Vault::create(Box::new(FolderStorage::new(&cloud))),
        Err(SyncError::VaultExists)
    ));
    assert_eq!(
        Vault::inspect(&FolderStorage::new(&cloud))
            .expect("inspect")
            .map(|i| i.vault_id),
        Some(a.vault.id().to_owned())
    );
    let empty = dir.path().join("empty");
    std::fs::create_dir_all(&empty).expect("dir");
    assert_eq!(
        Vault::inspect(&FolderStorage::new(&empty)).expect("inspect"),
        None
    );
}

#[test]
fn the_cloud_folder_holds_plain_files() {
    let dir = tempfile::tempdir().expect("tmp");
    let (a, _) = two_pcs(dir.path());
    a.write("slot1.sav", "a readable save");
    a.sync();
    let files = walk(&dir.path().join("cloud"));
    // The save itself, byte for byte, so a player can copy it back by hand.
    assert!(
        files
            .iter()
            .any(|p| std::fs::read_to_string(p).ok().as_deref() == Some("a readable save"))
    );
    // And a snapshot that says which file it is.
    let snapshot = files
        .iter()
        .find(|p| p.to_string_lossy().contains(&format!("games/{PORT}/")))
        .expect("snapshot");
    let text = std::fs::read_to_string(snapshot).expect("read");
    assert!(text.contains("slot1.sav"), "{text}");
}

#[test]
fn port_ids_cannot_escape_the_vault() {
    let dir = tempfile::tempdir().expect("tmp");
    let (a, _) = two_pcs(dir.path());
    assert!(matches!(
        a.syncer().sync("../oops", &a.rules, None),
        Err(SyncError::Rules(_))
    ));
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
            } else {
                out.push(e.path());
            }
        }
    }
    out
}
