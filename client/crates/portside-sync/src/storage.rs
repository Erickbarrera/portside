//! Where a vault lives. Only whole-file list, read, write and exists are needed, so any
//! storage works; the first is a folder kept in sync by the player's cloud app (ADR-0023).

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{IoContext, Result, SyncError};

/// Name of the vault folder inside the chosen location.
pub const VAULT_DIR: &str = "Portside Saves";

pub trait Storage: Send + Sync {
    /// A file's bytes, or None when it doesn't exist. `path` uses `/`.
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>>;
    /// Writes a whole file, replacing it, creating folders as needed. Readers never see a
    /// partly written file.
    fn write(&self, path: &str, bytes: &[u8]) -> Result<()>;
    fn exists(&self, path: &str) -> Result<bool>;
    /// Names of the files directly inside `dir` (empty when it doesn't exist).
    fn list(&self, dir: &str) -> Result<Vec<String>>;
    /// Where it is, for the UI.
    fn describe(&self) -> String;
}

/// A vault in `<location>/Portside Saves/`.
#[derive(Debug, Clone)]
pub struct FolderStorage {
    location: PathBuf,
    root: PathBuf,
}

impl FolderStorage {
    /// `location` is the folder the player chose (e.g. their Dropbox folder).
    pub fn new(location: impl Into<PathBuf>) -> FolderStorage {
        let location = location.into();
        FolderStorage {
            root: location.join(VAULT_DIR),
            location,
        }
    }

    pub fn location(&self) -> &Path {
        &self.location
    }

    fn path(&self, rel: &str) -> PathBuf {
        let mut path = self.root.clone();
        path.extend(rel.split('/').filter(|p| !p.is_empty()));
        path
    }

    fn check_location(&self) -> Result<()> {
        if self.location.is_dir() {
            Ok(())
        } else {
            Err(SyncError::Unavailable(format!(
                "{} isn't available. Check that the folder exists and its cloud app is running.",
                self.location.display()
            )))
        }
    }

    /// iCloud leaves `.name.icloud` in place of files it hasn't downloaded.
    fn placeholder(path: &Path) -> Option<PathBuf> {
        let name = path.file_name()?.to_str()?;
        Some(path.with_file_name(format!(".{name}.icloud")))
    }
}

impl Storage for FolderStorage {
    fn read(&self, rel: &str) -> Result<Option<Vec<u8>>> {
        self.check_location()?;
        let path = self.path(rel);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                match FolderStorage::placeholder(&path) {
                    Some(p) if p.exists() => Err(SyncError::NotDownloaded(rel.to_owned())),
                    _ => Ok(None),
                }
            }
            Err(e) => Err(e).at(&path),
        }
    }

    fn write(&self, rel: &str, bytes: &[u8]) -> Result<()> {
        self.check_location()?;
        let path = self.path(rel);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).at(dir)?;
        }
        let suffix = ulid::Ulid::new().random() & 0xffff_ffff_ffff;
        let tmp = path.with_extension(format!("{suffix:012x}.tmp"));
        let written = (|| {
            let mut file = std::fs::File::create(&tmp)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            std::fs::rename(&tmp, &path)
        })();
        if written.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        written.at(&path)
    }

    fn exists(&self, rel: &str) -> Result<bool> {
        self.check_location()?;
        let path = self.path(rel);
        Ok(path.is_file() || FolderStorage::placeholder(&path).is_some_and(|p| p.exists()))
    }

    fn list(&self, rel: &str) -> Result<Vec<String>> {
        self.check_location()?;
        let dir = self.path(rel);
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e).at(&dir),
        };
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry.at(&dir)?;
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let name = match name
                .strip_prefix('.')
                .and_then(|n| n.strip_suffix(".icloud"))
            {
                Some(real) => real.to_owned(),
                None if name.starts_with('.') || name.ends_with(".tmp") => continue,
                None => name,
            };
            names.push(name);
        }
        names.sort();
        names.dedup();
        Ok(names)
    }

    fn describe(&self) -> String {
        self.location.display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_replace_whole_files_and_list_skips_partial_ones() {
        let dir = tempfile::tempdir().expect("tmp");
        let storage = FolderStorage::new(dir.path());
        assert_eq!(storage.read("a/b.bin").expect("read"), None);
        storage.write("a/b.bin", b"one").expect("write");
        storage.write("a/b.bin", b"two").expect("write");
        assert_eq!(
            storage.read("a/b.bin").expect("read").as_deref(),
            Some(&b"two"[..])
        );
        std::fs::write(dir.path().join(VAULT_DIR).join("a/c.x.tmp"), b"").expect("tmp");
        std::fs::write(dir.path().join(VAULT_DIR).join("a/.d.bin.icloud"), b"").expect("ph");
        assert_eq!(storage.list("a").expect("list"), vec!["b.bin", "d.bin"]);
        assert!(matches!(
            storage.read("a/d.bin"),
            Err(SyncError::NotDownloaded(_))
        ));
        assert!(storage.list("missing").expect("list").is_empty());
    }

    #[test]
    fn a_missing_location_is_unavailable() {
        let storage = FolderStorage::new("/definitely/not/here");
        assert!(matches!(storage.list(""), Err(SyncError::Unavailable(_))));
    }
}
