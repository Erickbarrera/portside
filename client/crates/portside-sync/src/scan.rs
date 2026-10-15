//! Finding a game's save files on disk, and writing them back.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::error::{IoContext, Result, SyncError};
use crate::vault::MAX_FILE;

/// Temporary name while a restored file is written.
const TMP_SUFFIX: &str = ".portside-tmp";

/// Where a game's saves are and which files count (manifest `saves`, or the player's own
/// folder with every file).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveRules {
    pub root: PathBuf,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// A save file found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFile {
    pub abs: PathBuf,
    pub size: u64,
    pub mtime: i64,
}

struct Matcher {
    include: Option<GlobSet>,
    exclude: GlobSet,
}

fn globs(patterns: &[String]) -> Result<GlobSet> {
    let mut set = GlobSetBuilder::new();
    for p in patterns {
        set.add(Glob::new(p).map_err(|e| SyncError::Rules(format!("{p}: {e}")))?);
    }
    set.build().map_err(|e| SyncError::Rules(e.to_string()))
}

impl Matcher {
    fn new(rules: &SaveRules) -> Result<Matcher> {
        Ok(Matcher {
            include: if rules.include.is_empty() {
                None
            } else {
                Some(globs(&rules.include)?)
            },
            exclude: globs(&rules.exclude)?,
        })
    }

    fn matches(&self, rel: &str) -> bool {
        self.include.as_ref().is_none_or(|s| s.is_match(rel)) && !self.exclude.is_match(rel)
    }
}

fn mtime_of(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// The save files under `rules.root`, keyed by `/`-separated NFC path. A missing root is
/// empty. Symlinks aren't followed.
pub fn scan(rules: &SaveRules) -> Result<BTreeMap<String, LocalFile>> {
    let matcher = Matcher::new(rules)?;
    let mut found = BTreeMap::new();
    if !rules.root.is_dir() {
        return Ok(found);
    }
    let mut stack = vec![(rules.root.clone(), String::new())];
    while let Some((dir, prefix)) = stack.pop() {
        for entry in std::fs::read_dir(&dir).at(&dir)? {
            let entry = entry.at(&dir)?;
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let rel = format!("{prefix}{}", name.nfc());
            let kind = entry.file_type().at(&entry.path())?;
            if kind.is_dir() {
                stack.push((entry.path(), format!("{rel}/")));
            } else if kind.is_file() && !name.ends_with(TMP_SUFFIX) && matcher.matches(&rel) {
                let meta = entry.metadata().at(&entry.path())?;
                if meta.len() > MAX_FILE as u64 {
                    return Err(SyncError::TooLarge(rel));
                }
                found.insert(
                    rel,
                    LocalFile {
                        abs: entry.path(),
                        size: meta.len(),
                        mtime: mtime_of(&meta),
                    },
                );
            }
        }
    }
    Ok(found)
}

fn target(root: &Path, rel: &str) -> Result<PathBuf> {
    let mut path = root.to_path_buf();
    for part in rel.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(['\\', ':']) {
            return Err(SyncError::Corrupt(format!(
                "unsafe path in a snapshot: {rel}"
            )));
        }
        path.push(part);
    }
    Ok(path)
}

/// One file to write back.
pub struct Restore<'a> {
    pub rel: &'a str,
    pub bytes: Vec<u8>,
    pub mtime: i64,
}

/// Makes the save folder hold exactly `files` (of the files the rules cover): copies what
/// is there now to `trash` first, writes each file through a temporary name, and removes
/// covered files that aren't in `files`.
pub fn restore(
    rules: &SaveRules,
    current: &BTreeMap<String, LocalFile>,
    files: Vec<Restore<'_>>,
    trash: &Path,
) -> Result<()> {
    if !current.is_empty() {
        for (rel, file) in current {
            let copy = target(trash, rel)?;
            if let Some(dir) = copy.parent() {
                std::fs::create_dir_all(dir).at(dir)?;
            }
            std::fs::copy(&file.abs, &copy).at(&file.abs)?;
        }
    }
    let mut keep = std::collections::BTreeSet::new();
    for file in files {
        let path = target(&rules.root, file.rel)?;
        keep.insert(path.clone());
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).at(dir)?;
        }
        let tmp = path.with_file_name(format!(
            "{}{TMP_SUFFIX}",
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        ));
        let written = (|| {
            let mut out = std::fs::File::create(&tmp)?;
            out.write_all(&file.bytes)?;
            if file.mtime > 0 {
                out.set_modified(UNIX_EPOCH + Duration::from_secs(file.mtime as u64))?;
            }
            out.sync_all()?;
            drop(out);
            std::fs::rename(&tmp, &path)
        })();
        if written.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        written.at(&path)?;
    }
    for file in current.values() {
        if !keep.contains(&file.abs) {
            match std::fs::remove_file(&file.abs) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    return Err(e).at(&file.abs);
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Removes trash copies older than `max_age`. Errors are ignored: it's housekeeping.
pub fn prune_trash(trash_root: &Path, max_age: Duration) {
    let Ok(games) = std::fs::read_dir(trash_root) else {
        return;
    };
    let cutoff = SystemTime::now().checked_sub(max_age);
    for game in games.flatten() {
        let Ok(copies) = std::fs::read_dir(game.path()) else {
            continue;
        };
        for copy in copies.flatten() {
            let old = copy
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .zip(cutoff)
                .is_some_and(|(at, cutoff)| at < cutoff);
            if old {
                let _ = std::fs::remove_dir_all(copy.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_applies_include_and_exclude() {
        let dir = tempfile::tempdir().expect("tmp");
        let root = dir.path().join("saves");
        std::fs::create_dir_all(root.join("slot1/cache")).expect("dirs");
        std::fs::write(root.join("slot1/a.sav"), b"a").expect("a");
        std::fs::write(root.join("slot1/cache/b.sav"), b"b").expect("b");
        std::fs::write(root.join("game.log"), b"log").expect("log");
        std::fs::write(root.join("x.sav.portside-tmp"), b"tmp").expect("tmp");
        let rules = SaveRules {
            root: root.clone(),
            include: vec!["**/*.sav".to_owned()],
            exclude: vec!["**/cache/**".to_owned()],
        };
        let found = scan(&rules).expect("scan");
        assert_eq!(found.keys().collect::<Vec<_>>(), vec!["slot1/a.sav"]);
        let all = scan(&SaveRules {
            root,
            include: vec![],
            exclude: vec![],
        })
        .expect("scan");
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn restore_backs_up_writes_and_removes() {
        let dir = tempfile::tempdir().expect("tmp");
        let rules = SaveRules {
            root: dir.path().join("saves"),
            include: vec![],
            exclude: vec![],
        };
        std::fs::create_dir_all(&rules.root).expect("root");
        std::fs::write(rules.root.join("old.sav"), b"old").expect("old");
        std::fs::write(rules.root.join("keep.sav"), b"v1").expect("keep");
        let current = scan(&rules).expect("scan");
        let trash = dir.path().join("trash/1");
        restore(
            &rules,
            &current,
            vec![
                Restore {
                    rel: "keep.sav",
                    bytes: b"v2".to_vec(),
                    mtime: 1_700_000_000,
                },
                Restore {
                    rel: "sub/new.sav",
                    bytes: b"new".to_vec(),
                    mtime: 0,
                },
            ],
            &trash,
        )
        .expect("restore");
        let after = scan(&rules).expect("scan");
        assert_eq!(
            after.keys().collect::<Vec<_>>(),
            vec!["keep.sav", "sub/new.sav"]
        );
        assert_eq!(after["keep.sav"].mtime, 1_700_000_000);
        assert_eq!(
            std::fs::read(rules.root.join("keep.sav")).expect("r"),
            b"v2"
        );
        assert_eq!(std::fs::read(trash.join("old.sav")).expect("r"), b"old");
        assert_eq!(std::fs::read(trash.join("keep.sav")).expect("r"), b"v1");
        assert!(matches!(
            restore(
                &rules,
                &BTreeMap::new(),
                vec![Restore {
                    rel: "../escape",
                    bytes: vec![],
                    mtime: 0
                }],
                &trash
            ),
            Err(SyncError::Corrupt(_))
        ));
    }
}
