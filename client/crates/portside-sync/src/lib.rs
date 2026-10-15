//! Cloud saves: a vault of plain files in storage the player already has, synced before a
//! game starts and after it exits (docs/specs/save-sync-protocol.md, ADR-0023). No Portside
//! server is involved.

pub mod engine;
pub mod error;
pub mod scan;
pub mod state;
pub mod storage;
pub mod vault;

pub use engine::{Device, SnapshotInfo, SyncOutcome, SyncState, Syncer, now_rfc3339};
pub use error::{Result, SyncError};
pub use scan::{SaveRules, prune_trash};
pub use state::{GameRow, LocalState, SaveTemplate};
pub use storage::{FolderStorage, Storage, VAULT_DIR};
pub use vault::{DeviceRecord, Snapshot, Vault, VaultInfo};
