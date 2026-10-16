use std::path::PathBuf;

use thiserror::Error;

/// Everything that can stop an install. `src-tauri` maps each case to an error code.
#[derive(Debug, Error)]
pub enum InstallError {
    #[error("this port has no install information yet")]
    NoManifest,
    #[error("this port needs a newer Portside (manifest schema {0})")]
    UnsupportedSchema(u32),
    #[error("installing from {0} sources is not supported yet")]
    UnsupportedSource(String),
    #[error("not available for {os} {arch}")]
    NoBuild { os: String, arch: String },
    #[error("no release of {repo} has been published yet")]
    NoRelease { repo: String },
    #[error("release {tag} has no file matching {pattern}")]
    AssetMissing { tag: String, pattern: String },
    #[error("release {tag} has {count} files matching {pattern}, expected one")]
    AssetAmbiguous {
        tag: String,
        pattern: String,
        count: usize,
    },
    #[error("bad pattern {pattern}: {reason}")]
    BadPattern { pattern: String, reason: String },
    /// `reset_at` is Unix seconds, when GitHub says so.
    #[error("GitHub's hourly request limit was reached; try again later")]
    RateLimited { reset_at: Option<u64> },
    #[error("GitHub answered {status} for {what}")]
    GithubStatus { status: u16, what: String },
    #[error("downloads are only allowed from GitHub, not {0}")]
    HostNotAllowed(String),
    #[error("download failed with HTTP {0}")]
    DownloadStatus(u16),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("the downloaded file is {actual} bytes, expected {expected}")]
    SizeMismatch { expected: u64, actual: u64 },
    #[error("the downloaded file does not match its published checksum")]
    HashMismatch { expected: String, actual: String },
    #[error("the release publishes no checksum for {0}")]
    ChecksumMissing(String),
    #[error("{0} archives are not supported on this system yet")]
    UnsupportedArchive(String),
    #[error("the archive has {count} files matching {pattern}; expected one")]
    NestedArchive { pattern: String, count: usize },
    #[error("the archive is damaged: {0}")]
    BadArchive(String),
    #[error("the archive contains an unsafe path: {0}")]
    UnsafePath(String),
    #[error("the install has no {0}; the manifest may be out of date")]
    LaunchTargetMissing(String),
    #[error("this app is not signed by Apple and needs your OK before installing")]
    NeedsConsent,
    #[error("install cancelled")]
    Cancelled,
    #[error("{0} is not installed")]
    NotInstalled(String),
    /// The game's library folder is on storage that isn't there (an SD card taken out).
    #[error("the storage at {} isn't there", .0.display())]
    StorageMissing(PathBuf),
    #[error("{tool} failed: {message}")]
    Tool { tool: &'static str, message: String },
    #[error("file error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("library database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("library database migration failed: {0}")]
    Migration(#[from] refinery::Error),
    #[error("library data error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T, E = InstallError> = std::result::Result<T, E>;

/// Attaches the path to an `io::Error`.
pub(crate) trait IoContext<T> {
    fn at(self, path: impl Into<PathBuf>) -> Result<T>;
}

impl<T> IoContext<T> for std::io::Result<T> {
    fn at(self, path: impl Into<PathBuf>) -> Result<T> {
        self.map_err(|source| InstallError::Io {
            path: path.into(),
            source,
        })
    }
}
