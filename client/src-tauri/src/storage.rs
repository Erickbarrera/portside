//! Where games can be installed (client/docs/platforms.md, "Steam Deck"): this computer's
//! own storage, and on a Steam Deck the SD card while one is in.

use std::path::{Path, PathBuf};

use portside_core::paths;
use serde::{Deserialize, Serialize};
use specta::Type;
use sysinfo::Disks;

use crate::error::{CommandError, ErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum StorageKind {
    Internal,
    SdCard,
}

/// A place games can be installed.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Storage {
    pub kind: StorageKind,
    /// The library folder there.
    pub path: String,
    /// Free space in bytes, when known.
    pub free: Option<u64>,
}

/// This computer's own storage, then the SD card on a Steam Deck with one in.
pub fn list(internal: &Path) -> Vec<Storage> {
    let disks = Disks::new_with_refreshed_list();
    let entry = |kind, path: PathBuf| Storage {
        kind,
        free: free_space(&disks, &path),
        path: path.to_string_lossy().into_owned(),
    };
    let mut list = vec![entry(StorageKind::Internal, internal.to_path_buf())];
    if let Some(card) = paths::sd_card() {
        list.push(entry(StorageKind::SdCard, paths::sd_card_library(&card)));
    }
    list
}

/// The library folder to install into: None for the usual one.
pub fn library_for(kind: StorageKind) -> Result<Option<PathBuf>, CommandError> {
    match kind {
        StorageKind::Internal => Ok(None),
        StorageKind::SdCard => paths::sd_card()
            .map(|card| Some(paths::sd_card_library(&card)))
            .ok_or_else(|| CommandError {
                code: ErrorCode::StorageMissing,
                message: "There's no SD card in. Put one in, or install to internal storage."
                    .to_owned(),
            }),
    }
}

/// Free space on the drive holding `path` (the one mounted deepest above it).
fn free_space(disks: &Disks, path: &Path) -> Option<u64> {
    disks
        .iter()
        .filter(|d| path.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| d.available_space())
}
