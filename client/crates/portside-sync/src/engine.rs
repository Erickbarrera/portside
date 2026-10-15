//! Syncing one game (save-sync-protocol.md, "Sync").

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use portside_core::Os;
use serde::{Deserialize, Serialize};

use crate::error::{IoContext, Result, SyncError};
use crate::scan::{self, LocalFile, Restore, SaveRules};
use crate::state::LocalState;
use crate::vault::{DeviceRecord, FileEntry, Snapshot, Vault, same_files};

/// This device, as recorded in snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub os: Os,
    pub app_version: String,
}

/// What a sync did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    /// Nothing to do.
    UpToDate,
    /// This device's saves were sent to the vault.
    Uploaded,
    /// Saves from another device were written here.
    Restored,
    /// Two devices have different saves; the player has to keep one.
    Conflict,
}

/// A snapshot, as shown to the player.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SnapshotInfo {
    pub id: String,
    pub device_name: String,
    pub os: Os,
    pub this_device: bool,
    /// RFC 3339.
    pub created_at: String,
    pub file_count: u32,
    pub size: u64,
    pub port_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SyncOutcome {
    pub state: SyncState,
    /// The snapshot this device now matches.
    pub current: Option<SnapshotInfo>,
    /// The sides of a conflict, newest first.
    pub conflict: Vec<SnapshotInfo>,
}

pub struct Syncer<'a> {
    pub vault: &'a Vault,
    pub state: &'a LocalState,
    pub device: &'a Device,
    /// `{app_data}/saves-trash`: a copy of the saves is put here before a restore.
    pub trash_root: &'a Path,
}

/// RFC 3339 in UTC, to the second.
pub fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default();
    rfc3339(secs)
}

fn rfc3339(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Days to civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// Snapshots nobody builds on, oldest first.
fn current_snapshots(snapshots: &[Snapshot]) -> Vec<&Snapshot> {
    let parents: BTreeSet<&str> = snapshots
        .iter()
        .flat_map(|s| s.parents.iter().map(String::as_str))
        .collect();
    snapshots
        .iter()
        .filter(|s| !parents.contains(s.id.as_str()))
        .collect()
}

/// One snapshot per distinct set of files (the newest), newest first.
fn distinct(leaves: &[&Snapshot]) -> Vec<Snapshot> {
    let mut out: Vec<Snapshot> = Vec::new();
    for leaf in leaves.iter().rev() {
        if !out.iter().any(|s| s.same_files(leaf)) {
            out.push((*leaf).clone());
        }
    }
    out
}

fn safe_name(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

impl Syncer<'_> {
    fn info(&self, s: &Snapshot) -> SnapshotInfo {
        SnapshotInfo {
            id: s.id.clone(),
            device_name: s.device_name.clone(),
            os: s.os,
            this_device: s.device_id == self.device.id,
            created_at: s.created_at.clone(),
            file_count: s.files.len() as u32,
            size: s.total_size(),
            port_version: s.port_version.clone(),
        }
    }

    /// The save folder as file entries. Every file is hashed: a size and time check would
    /// miss a save rewritten within the same second as a restore.
    fn local_entries(
        &self,
        local: &BTreeMap<String, LocalFile>,
    ) -> Result<BTreeMap<String, FileEntry>> {
        let mut entries = BTreeMap::new();
        for (rel, file) in local {
            let blob = Vault::blob_id(&std::fs::read(&file.abs).at(&file.abs)?);
            entries.insert(
                rel.clone(),
                FileEntry {
                    size: file.size,
                    mtime: file.mtime,
                    blob,
                },
            );
        }
        Ok(entries)
    }

    fn new_snapshot(
        &self,
        port_id: &str,
        parents: Vec<String>,
        files: BTreeMap<String, FileEntry>,
        port_version: Option<&str>,
    ) -> Snapshot {
        Snapshot {
            id: ulid::Ulid::new().to_string(),
            port_id: port_id.to_owned(),
            device_id: self.device.id.clone(),
            device_name: self.device.name.clone(),
            os: self.device.os,
            created_at: now_rfc3339(),
            parents,
            port_version: port_version.map(str::to_owned),
            files,
        }
    }

    /// Steps 1 and 2: sends the save folder to the vault when it changed since the base.
    /// Returns the folder's files and whether a snapshot was written.
    fn upload_changes(
        &self,
        port_id: &str,
        rules: &SaveRules,
        port_version: Option<&str>,
    ) -> Result<(BTreeMap<String, LocalFile>, bool)> {
        let game = self.state.game(port_id)?;
        let local = scan::scan(rules)?;
        let entries = self.local_entries(&local)?;
        if same_files(&entries, &game.base_files) {
            return Ok((local, false));
        }
        let known: BTreeSet<&str> = game.base_files.values().map(|f| f.blob.as_str()).collect();
        let mut entries = entries;
        for (rel, entry) in entries.iter_mut() {
            if known.contains(entry.blob.as_str()) {
                continue;
            }
            let file = &local[rel];
            let bytes = std::fs::read(&file.abs).at(&file.abs)?;
            // The file may have changed since it was hashed; store what was read.
            entry.blob = self.vault.put_blob(&bytes)?;
            entry.size = bytes.len() as u64;
        }
        let snapshot = self.new_snapshot(
            port_id,
            game.base_id.into_iter().collect(),
            entries,
            port_version,
        );
        self.vault.write_snapshot(&snapshot)?;
        self.state
            .set_base(port_id, &snapshot.id, &snapshot.files)?;
        Ok((local, true))
    }

    /// Makes the save folder match `snapshot`, keeping a copy of what was there.
    fn restore(
        &self,
        port_id: &str,
        rules: &SaveRules,
        current: &BTreeMap<String, LocalFile>,
        snapshot: &Snapshot,
    ) -> Result<()> {
        // Fetch and check everything before touching the folder.
        let mut files = Vec::with_capacity(snapshot.files.len());
        for (rel, entry) in &snapshot.files {
            files.push(Restore {
                rel,
                bytes: self.vault.get_blob(&entry.blob)?,
                mtime: entry.mtime,
            });
        }
        let trash: PathBuf = self
            .trash_root
            .join(safe_name(port_id))
            .join(now_rfc3339().replace(':', "-"));
        scan::restore(rules, current, files, &trash)
    }

    fn record_device(&self) {
        let record = DeviceRecord {
            id: self.device.id.clone(),
            name: self.device.name.clone(),
            os: self.device.os,
            app_version: self.device.app_version.clone(),
            last_sync: now_rfc3339(),
        };
        if let Err(e) = self.vault.write_device(&record) {
            eprintln!("could not update this device's record: {e}");
        }
    }

    fn finish(&self, port_id: &str, result: Result<SyncOutcome>) -> Result<SyncOutcome> {
        let error = result.as_ref().err().map(ToString::to_string);
        if let Err(e) = self.state.set_result(port_id, unix_now(), error.as_deref()) {
            eprintln!("could not record the sync of {port_id}: {e}");
        }
        if result.is_ok() {
            self.record_device();
        }
        result
    }

    /// Syncs one game: uploads local changes, then restores another device's newer saves
    /// or reports a conflict.
    pub fn sync(
        &self,
        port_id: &str,
        rules: &SaveRules,
        port_version: Option<&str>,
    ) -> Result<SyncOutcome> {
        let result = self.sync_inner(port_id, rules, port_version);
        self.finish(port_id, result)
    }

    fn sync_inner(
        &self,
        port_id: &str,
        rules: &SaveRules,
        port_version: Option<&str>,
    ) -> Result<SyncOutcome> {
        let (local, uploaded) = self.upload_changes(port_id, rules, port_version)?;
        let snapshots = self.vault.snapshots(port_id)?;
        let leaves = current_snapshots(&snapshots);
        let sides = distinct(&leaves);
        let base_id = self.state.game(port_id)?.base_id;
        let base = base_id
            .as_deref()
            .and_then(|id| snapshots.iter().find(|s| s.id == id));
        match sides.as_slice() {
            [] => Ok(SyncOutcome {
                state: if uploaded {
                    SyncState::Uploaded
                } else {
                    SyncState::UpToDate
                },
                current: base.map(|b| self.info(b)),
                conflict: Vec::new(),
            }),
            [one] => {
                let base_files = self.state.game(port_id)?.base_files;
                let state = if same_files(&one.files, &base_files) {
                    if uploaded {
                        SyncState::Uploaded
                    } else {
                        SyncState::UpToDate
                    }
                } else {
                    self.restore(port_id, rules, &local, one)?;
                    SyncState::Restored
                };
                if base_id.as_deref() != Some(one.id.as_str()) {
                    self.state.set_base(port_id, &one.id, &one.files)?;
                }
                Ok(SyncOutcome {
                    state,
                    current: Some(self.info(one)),
                    conflict: Vec::new(),
                })
            }
            many => Ok(SyncOutcome {
                state: SyncState::Conflict,
                current: base.map(|b| self.info(b)),
                conflict: many.iter().map(|s| self.info(s)).collect(),
            }),
        }
    }

    /// Settles a conflict by keeping `snapshot_id`'s saves on every device.
    pub fn keep(
        &self,
        port_id: &str,
        rules: &SaveRules,
        snapshot_id: &str,
        port_version: Option<&str>,
    ) -> Result<SyncOutcome> {
        let result = self.keep_inner(port_id, rules, snapshot_id, port_version);
        self.finish(port_id, result)
    }

    fn keep_inner(
        &self,
        port_id: &str,
        rules: &SaveRules,
        snapshot_id: &str,
        port_version: Option<&str>,
    ) -> Result<SyncOutcome> {
        // Anything played since the conflict was shown becomes a side too, so it's kept
        // in the history even if the player picks the other one.
        let (local, _) = self.upload_changes(port_id, rules, port_version)?;
        let snapshots = self.vault.snapshots(port_id)?;
        let leaves = current_snapshots(&snapshots);
        let chosen = snapshots
            .iter()
            .find(|s| s.id == snapshot_id)
            .filter(|chosen| leaves.iter().any(|l| l.same_files(chosen)))
            .ok_or(SyncError::UnknownSnapshot)?;
        let merged = self.new_snapshot(
            port_id,
            leaves.iter().map(|l| l.id.clone()).collect(),
            chosen.files.clone(),
            port_version,
        );
        let base_files = self.state.game(port_id)?.base_files;
        let restored = !same_files(&chosen.files, &base_files);
        if restored {
            self.restore(port_id, rules, &local, chosen)?;
        }
        self.vault.write_snapshot(&merged)?;
        self.state.set_base(port_id, &merged.id, &merged.files)?;
        Ok(SyncOutcome {
            state: if restored {
                SyncState::Restored
            } else {
                SyncState::Uploaded
            },
            current: Some(self.info(&merged)),
            conflict: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_rfc3339() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_792_000_000), "2026-10-14T17:46:40Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
    }
}
