//! A save vault: `vault.json`, device records, snapshots and blobs, all plain files the
//! player can open (save-sync-protocol.md, "Vault layout").

use std::collections::BTreeMap;

use portside_core::Os;
use serde::{Deserialize, Serialize};

use crate::error::{Result, SyncError};
use crate::storage::Storage;

const FORMAT: &str = "portside-saves";
const VERSION: u32 = 2;
/// Largest file that syncs.
pub const MAX_FILE: usize = 256 * 1024 * 1024;
/// Largest snapshot or device record.
const MAX_RECORD: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VaultFile {
    format: String,
    version: u32,
    vault_id: String,
}

/// One save file in a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub size: u64,
    /// Unix seconds.
    pub mtime: i64,
    pub blob: String,
}

/// Every save file of a game at one moment. Immutable once written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub port_id: String,
    pub device_id: String,
    pub device_name: String,
    pub os: Os,
    /// RFC 3339.
    pub created_at: String,
    pub parents: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_version: Option<String>,
    /// Path relative to the save root → file.
    pub files: BTreeMap<String, FileEntry>,
}

impl Snapshot {
    pub fn total_size(&self) -> u64 {
        self.files.values().map(|f| f.size).sum()
    }

    /// Same paths with the same contents.
    pub fn same_files(&self, other: &Snapshot) -> bool {
        same_files(&self.files, &other.files)
    }
}

pub(crate) fn same_files(a: &BTreeMap<String, FileEntry>, b: &BTreeMap<String, FileEntry>) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|((pa, fa), (pb, fb))| pa == pb && fa.blob == fb.blob)
}

/// A device that uses the vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DeviceRecord {
    pub id: String,
    pub name: String,
    pub os: Os,
    pub app_version: String,
    /// RFC 3339.
    pub last_sync: String,
}

/// How long one device has played one game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaytimeEntry {
    pub total_secs: u64,
    /// Unix seconds the last session ended.
    pub last_played: u64,
    pub sessions: u64,
}

/// A device's play time, in `playtime/<device id>.json`. Only that device writes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaytimeRecord {
    pub device_id: String,
    pub device_name: String,
    /// RFC 3339.
    pub updated_at: String,
    /// Port id → play time.
    pub games: BTreeMap<String, PlaytimeEntry>,
}

/// What `vault.json` says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultInfo {
    pub vault_id: String,
}

pub struct Vault {
    storage: Box<dyn Storage>,
    id: String,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault")
            .field("id", &self.id)
            .field("at", &self.storage.describe())
            .finish()
    }
}

fn read_vault_file(storage: &dyn Storage) -> Result<Option<VaultFile>> {
    let Some(bytes) = storage.read("vault.json")? else {
        return Ok(None);
    };
    let file: VaultFile = serde_json::from_slice(&bytes)
        .map_err(|e| SyncError::Corrupt(format!("vault.json: {e}")))?;
    if file.format != FORMAT {
        return Err(SyncError::Corrupt(
            "vault.json isn't a Portside vault".to_owned(),
        ));
    }
    if file.version > VERSION {
        return Err(SyncError::NewerVault);
    }
    Ok(Some(file))
}

/// A catalog port id: `a-z`, `0-9`, `-`, `_`, `.`, not starting with `.`.
fn safe_port_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "-_.".contains(c))
}

/// A device id (a ULID), safe as a file name.
fn safe_device_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric())
}

impl Vault {
    /// Whether `storage` holds a vault already.
    pub fn inspect(storage: &dyn Storage) -> Result<Option<VaultInfo>> {
        Ok(read_vault_file(storage)?.map(|f| VaultInfo {
            vault_id: f.vault_id,
        }))
    }

    /// Opens the vault in `storage`, making a new one if there is none.
    pub fn open_or_create(storage: Box<dyn Storage>) -> Result<Vault> {
        if read_vault_file(storage.as_ref())?.is_some() {
            Vault::open(storage)
        } else {
            Vault::create(storage)
        }
    }

    /// Opens the vault in `storage`.
    pub fn open(storage: Box<dyn Storage>) -> Result<Vault> {
        let file = read_vault_file(storage.as_ref())?.ok_or(SyncError::NoVault)?;
        Ok(Vault {
            storage,
            id: file.vault_id,
        })
    }

    /// Makes a new, empty vault. Fails if one is there already.
    pub fn create(storage: Box<dyn Storage>) -> Result<Vault> {
        if read_vault_file(storage.as_ref())?.is_some() {
            return Err(SyncError::VaultExists);
        }
        let id = ulid::Ulid::new().to_string();
        let file = VaultFile {
            format: FORMAT.to_owned(),
            version: VERSION,
            vault_id: id.clone(),
        };
        storage.write("vault.json", &serde_json::to_vec_pretty(&file)?)?;
        Ok(Vault { storage, id })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn describe(&self) -> String {
        self.storage.describe()
    }

    fn read_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<Option<T>> {
        let Some(bytes) = self.storage.read(path)? else {
            return Ok(None);
        };
        if bytes.len() > MAX_RECORD {
            return Err(SyncError::Corrupt(path.to_owned()));
        }
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| SyncError::Corrupt(path.to_owned()))
    }

    fn write_json<T: Serialize>(&self, path: &str, value: &T) -> Result<()> {
        self.storage.write(path, &serde_json::to_vec_pretty(value)?)
    }

    pub fn write_device(&self, device: &DeviceRecord) -> Result<()> {
        self.write_json(&format!("devices/{}.json", device.id), device)
    }

    /// Every device that has used the vault. Unreadable records are skipped.
    pub fn devices(&self) -> Result<Vec<DeviceRecord>> {
        let mut devices = Vec::new();
        for name in self.storage.list("devices")? {
            if !name.ends_with(".json") {
                continue;
            }
            if let Ok(Some(d)) = self.read_json::<DeviceRecord>(&format!("devices/{name}")) {
                devices.push(d);
            }
        }
        devices.sort_by(|a, b| b.last_sync.cmp(&a.last_sync));
        Ok(devices)
    }

    /// Rewrites this device's play time.
    pub fn write_playtime(&self, record: &PlaytimeRecord) -> Result<()> {
        if !safe_device_id(&record.device_id) {
            return Err(SyncError::Rules(format!(
                "{:?} isn't a device id",
                record.device_id
            )));
        }
        self.write_json(&format!("playtime/{}.json", record.device_id), record)
    }

    /// Every device's play time. Files that don't open or don't match their name are
    /// skipped, and so are games that aren't port ids.
    pub fn playtime(&self) -> Result<Vec<PlaytimeRecord>> {
        let mut found = Vec::new();
        for name in self.storage.list("playtime")? {
            let Some(device) = name.strip_suffix(".json") else {
                continue;
            };
            match self.read_json::<PlaytimeRecord>(&format!("playtime/{name}")) {
                Ok(Some(mut r)) if r.device_id == device => {
                    r.games.retain(|port_id, _| safe_port_id(port_id));
                    found.push(r);
                }
                Ok(_) | Err(SyncError::NotDownloaded(_)) | Err(SyncError::Corrupt(_)) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(found)
    }

    fn snapshot_dir(port_id: &str) -> Result<String> {
        if !safe_port_id(port_id) {
            return Err(SyncError::Rules(format!("{port_id:?} isn't a port id")));
        }
        Ok(format!("games/{port_id}"))
    }

    pub fn write_snapshot(&self, snapshot: &Snapshot) -> Result<()> {
        let path = format!(
            "{}/{}.{}.json",
            Vault::snapshot_dir(&snapshot.port_id)?,
            snapshot.id,
            snapshot.device_id
        );
        self.write_json(&path, snapshot)
    }

    /// Every snapshot of a game. Snapshots another device is still uploading, or that
    /// don't open, are skipped.
    pub fn snapshots(&self, port_id: &str) -> Result<Vec<Snapshot>> {
        let dir = Vault::snapshot_dir(port_id)?;
        let mut found = Vec::new();
        for name in self.storage.list(&dir)? {
            if !name.ends_with(".json") {
                continue;
            }
            match self.read_json::<Snapshot>(&format!("{dir}/{name}")) {
                Ok(Some(s)) if s.port_id == port_id && name.starts_with(&s.id) => found.push(s),
                Ok(_) | Err(SyncError::NotDownloaded(_)) | Err(SyncError::Corrupt(_)) => {}
                Err(e) => return Err(e),
            }
        }
        found.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(found)
    }

    fn blob_path(id: &str) -> String {
        format!("blobs/{}/{id}", id.get(..2).unwrap_or("00"))
    }

    /// Content id of a file's bytes.
    pub fn blob_id(bytes: &[u8]) -> String {
        blake3::hash(bytes).to_hex().to_string()
    }

    /// Stores a file's bytes unless the vault has them already. Returns the blob id.
    pub fn put_blob(&self, bytes: &[u8]) -> Result<String> {
        let id = Vault::blob_id(bytes);
        let path = Vault::blob_path(&id);
        if !self.storage.exists(&path)? {
            self.storage.write(&path, bytes)?;
        }
        Ok(id)
    }

    /// A file's bytes, checked against its id.
    pub fn get_blob(&self, id: &str) -> Result<Vec<u8>> {
        let path = Vault::blob_path(id);
        let bytes = self
            .storage
            .read(&path)?
            .ok_or_else(|| SyncError::Unavailable(format!(
                "A save file isn't in the folder yet ({path}). Wait for your cloud app to finish syncing, then try again."
            )))?;
        if bytes.len() > MAX_FILE || Vault::blob_id(&bytes) != id {
            return Err(SyncError::Corrupt(path));
        }
        Ok(bytes)
    }
}
