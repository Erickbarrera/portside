use std::path::{Path, PathBuf};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, SyncError>;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The storage folder isn't there (unplugged drive, cloud app not running).
    #[error("{0}")]
    Unavailable(String),
    /// The cloud app hasn't downloaded a file yet (e.g. iCloud "Optimise Mac Storage").
    #[error(
        "{0} isn't on this computer yet. Open the folder so your cloud app downloads it, then try again."
    )]
    NotDownloaded(String),
    #[error("There are no Portside saves in that folder yet.")]
    NoVault,
    #[error("Portside saves are already in that folder.")]
    VaultExists,
    #[error("This vault was made by a newer Portside. Update Portside to use it.")]
    NewerVault,
    #[error("A saved file is damaged: {0}")]
    Corrupt(String),
    #[error("{0} is too large to sync (over 256 MB).")]
    TooLarge(String),
    #[error("Cloud saves aren't set up on this device.")]
    NotEnabled,
    #[error("No save folder is known for this game on this system.")]
    NoSaveFolder,
    #[error("That snapshot isn't one of the sides of the conflict.")]
    UnknownSnapshot,
    #[error("Bad save rules: {0}")]
    Rules(String),
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
    #[error(transparent)]
    Migration(#[from] refinery::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Attaches a path to I/O errors.
pub(crate) trait IoContext<T> {
    fn at(self, path: &Path) -> Result<T>;
}

impl<T> IoContext<T> for std::io::Result<T> {
    fn at(self, path: &Path) -> Result<T> {
        self.map_err(|source| SyncError::Io {
            path: path.to_path_buf(),
            source,
        })
    }
}
