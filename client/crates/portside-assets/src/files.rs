//! Matching a port's `files` against a folder, and copying them.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use crate::AssetsError;

/// How far below the store's game folder the files may be (a macOS app bundle's
/// `Contents/Resources/game`, say).
const MAX_DEPTH: usize = 4;
/// Folders looked at, at most, while searching one game folder.
const MAX_FOLDERS: usize = 4000;

/// `rel` inside `base`, matching each name without regard to case (an exact match wins).
/// `rel` ending in `/` must be a folder, otherwise a file.
fn lookup(base: &Path, rel: &str) -> Option<PathBuf> {
    let want_dir = rel.ends_with('/');
    let mut path = base.to_path_buf();
    for part in rel.split('/').filter(|p| !p.is_empty()) {
        if part == ".." || part == "." {
            return None;
        }
        let exact = path.join(part);
        if std::fs::symlink_metadata(&exact).is_ok() {
            path = exact;
            continue;
        }
        let entry = std::fs::read_dir(&path)
            .ok()?
            .flatten()
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))?;
        path = entry.path();
    }
    let meta = std::fs::metadata(&path).ok()?;
    (meta.is_dir() == want_dir).then_some(path)
}

/// True when `dir` holds every one of `files`.
pub fn has_files(dir: &Path, files: &[String]) -> bool {
    !files.is_empty() && files.iter().all(|f| lookup(dir, f).is_some())
}

/// The shallowest folder in `dir` (itself included, up to four levels down) that holds every
/// one of `files`. Links to other folders aren't followed.
pub fn data_root(dir: &Path, files: &[String]) -> Option<PathBuf> {
    let mut queue = VecDeque::from([(dir.to_path_buf(), 0)]);
    let mut seen = 0;
    while let Some((folder, depth)) = queue.pop_front() {
        if has_files(&folder, files) {
            return Some(folder);
        }
        seen += 1;
        if depth == MAX_DEPTH || seen > MAX_FOLDERS {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        let mut subs: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.path())
            .collect();
        subs.sort();
        queue.extend(subs.into_iter().map(|p| (p, depth + 1)));
    }
    None
}

/// Copies `files` from `root` into `dest`, keeping the spelling the copy uses, and folders
/// with everything in them. Returns the bytes copied.
pub fn copy_files(root: &Path, files: &[String], dest: &Path) -> Result<u64, AssetsError> {
    let mut bytes = 0;
    for f in files {
        let src = lookup(root, f).ok_or(AssetsError::NotFound)?;
        let rel = src.strip_prefix(root).map_err(|_| AssetsError::NotFound)?;
        let target = dest.join(rel);
        if src.is_dir() {
            bytes += copy_dir(&src, &target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            bytes += std::fs::copy(&src, &target)?;
        }
    }
    Ok(bytes)
}

fn copy_dir(src: &Path, dest: &Path) -> Result<u64, AssetsError> {
    std::fs::create_dir_all(dest)?;
    let mut bytes = 0;
    for entry in std::fs::read_dir(src)?.flatten() {
        let kind = entry.file_type()?;
        let to = dest.join(entry.file_name());
        if kind.is_dir() {
            bytes += copy_dir(&entry.path(), &to)?;
        } else if kind.is_file() {
            bytes += std::fs::copy(entry.path(), &to)?;
        }
        // Links are left out: they could point anywhere.
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn make(root: &Path, files: &[&str]) -> std::io::Result<()> {
        for f in files {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap_or(root))?;
            std::fs::write(p, b"synthetic")?;
        }
        Ok(())
    }

    #[test]
    fn matches_names_without_regard_to_case() -> Result<(), Box<dyn std::error::Error>> {
        let game = tempfile::tempdir()?;
        make(game.path(), &["MASTER.DAT", "Data/Sound/a.acm"])?;
        assert!(has_files(game.path(), &names(&["master.dat", "data/"])));
        assert!(!has_files(game.path(), &names(&["master.dat/"])));
        assert!(!has_files(game.path(), &names(&["data"])));
        assert!(!has_files(game.path(), &names(&["missing.dat"])));
        assert!(!has_files(game.path(), &names(&["../x"])));
        assert!(!has_files(game.path(), &[]));
        Ok(())
    }

    #[test]
    fn finds_the_shallowest_folder_with_every_file() -> Result<(), Box<dyn std::error::Error>> {
        let game = tempfile::tempdir()?;
        make(
            game.path(),
            &[
                "Example.app/Contents/Resources/game/data.pak",
                "Example.app/Contents/Resources/game/music/1.ogg",
                // Only one of the two here.
                "Example.app/Contents/data.pak",
            ],
        )?;
        let files = names(&["data.pak", "music/"]);
        assert_eq!(
            data_root(game.path(), &files),
            Some(game.path().join("Example.app/Contents/Resources/game"))
        );
        make(
            game.path(),
            &["deep/a/b/c/d/e/data.pak", "deep/a/b/c/d/e/music/1.ogg"],
        )?;
        assert_eq!(data_root(&game.path().join("deep"), &files), None);
        Ok(())
    }

    #[test]
    fn copies_files_and_folders_with_their_spelling() -> Result<(), Box<dyn std::error::Error>> {
        let game = tempfile::tempdir()?;
        let dest = tempfile::tempdir()?;
        make(
            game.path(),
            &["MASTER.DAT", "DATA/Sound/a.acm", "DATA/b.txt"],
        )?;
        let files = names(&["master.dat", "data/"]);
        let bytes = copy_files(game.path(), &files, dest.path())?;
        assert_eq!(bytes, 3 * 9);
        assert!(dest.path().join("MASTER.DAT").is_file());
        assert!(dest.path().join("DATA/Sound/a.acm").is_file());
        assert!(has_files(dest.path(), &files));
        assert!(matches!(
            copy_files(game.path(), &names(&["nope.dat"]), dest.path()),
            Err(AssetsError::NotFound)
        ));
        Ok(())
    }
}
