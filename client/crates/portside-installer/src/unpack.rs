//! Unpacks a downloaded file into a fresh folder. Blocking; run it off the async runtime.
//!
//! Archives come from upstream projects, not from us, so every entry is checked: no absolute
//! paths, no `..`, and nothing is written through a symlink the archive itself created.

use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::{Component, Path, PathBuf};

use portside_api::models::ArchiveKind;

use crate::error::{InstallError, IoContext, Result};

/// Files some tools add to archives that are never part of the app.
const JUNK: &[&str] = &["__MACOSX", ".DS_Store"];

/// Unpacks `file` into `out`, which must not exist yet. `save_as` names the result when
/// `kind` is `none`, `appimage` or `apk` (the file is used as is; Android installs the APK). Returns the folder holding the
/// app: `out` itself, or with `strip_root` its single top-level folder.
pub fn unpack(
    kind: ArchiveKind,
    file: &Path,
    save_as: &str,
    out: &Path,
    strip_root: bool,
) -> Result<PathBuf> {
    fs::create_dir_all(out).at(out)?;
    match kind {
        ArchiveKind::Zip => unzip(file, out)?,
        ArchiveKind::TarGz => untar(flate2::read::GzDecoder::new(open(file)?), out)?,
        ArchiveKind::TarXz => untar(liblzma::read::XzDecoder::new(open(file)?), out)?,
        ArchiveKind::Dmg => undmg(file, out)?,
        ArchiveKind::SevenZ => un7z(file, out)?,
        ArchiveKind::None | ArchiveKind::AppImage | ArchiveKind::Apk => {
            let dest = prepare(out, &safe_relative(save_as)?)?;
            fs::copy(file, &dest).at(&dest)?;
            if kind == ArchiveKind::AppImage {
                make_executable(&dest)?;
            }
        }
    }
    for junk in JUNK {
        let p = out.join(junk);
        match fs::symlink_metadata(&p) {
            Ok(m) if m.is_dir() => fs::remove_dir_all(&p).at(&p)?,
            Ok(_) => fs::remove_file(&p).at(&p)?,
            Err(_) => {}
        }
    }
    if strip_root {
        let entries: Vec<_> = fs::read_dir(out)
            .at(out)?
            .collect::<io::Result<_>>()
            .at(out)?;
        if let [only] = entries.as_slice()
            && only.file_type().at(only.path())?.is_dir()
        {
            return Ok(only.path());
        }
    }
    Ok(out.to_path_buf())
}

/// The one regular file below `root` whose relative path (with `/` separators) matches
/// `pattern`, for a `nested` archive. Symlinks are not followed.
pub fn find_nested(root: &Path, pattern: &str) -> Result<PathBuf> {
    let glob = globset::Glob::new(pattern)
        .map_err(|e| InstallError::BadPattern {
            pattern: pattern.to_owned(),
            reason: e.kind().to_string(),
        })?
        .compile_matcher();
    let mut hits = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).at(&dir)? {
            let entry = entry.at(&dir)?;
            let kind = entry.file_type().at(entry.path())?;
            if kind.is_dir() {
                dirs.push(entry.path());
            } else if kind.is_file() {
                let path = entry.path();
                let rel = path.strip_prefix(root).unwrap_or(&path);
                let rel: Vec<_> = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect();
                if glob.is_match(rel.join("/")) {
                    hits.push(path);
                }
            }
        }
    }
    match <[PathBuf; 1]>::try_from(hits) {
        Ok([one]) => Ok(one),
        Err(hits) => Err(InstallError::NestedArchive {
            pattern: pattern.to_owned(),
            count: hits.len(),
        }),
    }
}

/// Sets the executable bits on a file (no-op off Unix).
pub fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).at(path)?.permissions();
        perms.set_mode(perms.mode() | 0o111);
        fs::set_permissions(path, perms).at(path)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn open(file: &Path) -> Result<BufReader<File>> {
    Ok(BufReader::new(File::open(file).at(file)?))
}

fn bad(e: impl std::fmt::Display) -> InstallError {
    InstallError::BadArchive(e.to_string())
}

/// A relative path with only normal components.
fn safe_relative(name: &str) -> Result<PathBuf> {
    let path = Path::new(name);
    let mut clean = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => return Err(InstallError::UnsafePath(name.to_owned())),
        }
    }
    if clean.as_os_str().is_empty() {
        return Err(InstallError::UnsafePath(name.to_owned()));
    }
    Ok(clean)
}

/// Fails when `rel` (or a folder above it, below `root`) is a symlink, so writes can't be
/// redirected outside the install folder.
fn guard_no_links(root: &Path, rel: &Path) -> Result<()> {
    let mut at = root.to_path_buf();
    for part in rel.components() {
        at.push(part);
        if fs::symlink_metadata(&at).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(InstallError::UnsafePath(rel.display().to_string()));
        }
    }
    Ok(())
}

fn prepare(root: &Path, rel: &Path) -> Result<PathBuf> {
    guard_no_links(root, rel)?;
    let dest = root.join(rel);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).at(parent)?;
    }
    Ok(dest)
}

fn write_link(root: &Path, rel: &Path, target: &str) -> Result<()> {
    if target.is_empty() || Path::new(target).is_absolute() {
        return Err(InstallError::UnsafePath(format!(
            "{} -> {target}",
            rel.display()
        )));
    }
    let dest = prepare(root, rel)?;
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, &dest).at(&dest)?;
    // Symlinks only appear in macOS and Linux builds; Windows builds don't use them.
    #[cfg(not(unix))]
    let _ = dest;
    Ok(())
}

fn write_file(root: &Path, rel: &Path, mut from: impl io::Read, mode: Option<u32>) -> Result<()> {
    let dest = prepare(root, rel)?;
    let mut to = File::create(&dest).at(&dest)?;
    io::copy(&mut from, &mut to).at(&dest)?;
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dest, fs::Permissions::from_mode(mode & 0o777)).at(&dest)?;
    }
    #[cfg(not(unix))]
    let _ = mode;
    Ok(())
}

fn unzip(file: &Path, out: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(open(file)?).map_err(bad)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(bad)?;
        let rel = entry.enclosed_name().ok_or_else(|| {
            InstallError::UnsafePath(entry.name().map(|n| n.into_owned()).unwrap_or_default())
        })?;
        let rel = safe_relative(&rel.to_string_lossy())?;
        if entry.is_dir() {
            guard_no_links(out, &rel)?;
            let dest = out.join(&rel);
            fs::create_dir_all(&dest).at(&dest)?;
        } else if entry.is_symlink() {
            let mut target = String::new();
            io::Read::read_to_string(&mut entry, &mut target).map_err(bad)?;
            write_link(out, &rel, &target)?;
        } else {
            let mode = entry.unix_mode();
            write_file(out, &rel, &mut entry, mode)?;
        }
    }
    Ok(())
}

fn un7z(file: &Path, out: &Path) -> Result<()> {
    let mut archive =
        sevenz_rust2::ArchiveReader::new(open(file)?, sevenz_rust2::Password::empty())
            .map_err(bad)?;
    let mut failed = None;
    archive
        .for_each_entries(|entry, data| match un7z_entry(entry, data, out) {
            Ok(()) => Ok(true),
            Err(e) => {
                failed = Some(e);
                Ok(false)
            }
        })
        .map_err(bad)?;
    failed.map_or(Ok(()), Err)
}

fn un7z_entry(
    entry: &sevenz_rust2::ArchiveEntry,
    data: &mut dyn io::Read,
    out: &Path,
) -> Result<()> {
    if entry.is_anti_item() {
        return Ok(());
    }
    // Archives made on Windows may use `\` between folders.
    let raw = entry.name().replace('\\', "/");
    let rel = safe_relative(&raw)?;
    // 7-Zip keeps a Unix mode in the high bits of the attributes when it has one.
    let attrs = entry.windows_attributes();
    let mode = (attrs & 0x8000 != 0).then_some(attrs >> 16);
    if entry.is_directory() {
        guard_no_links(out, &rel)?;
        let dest = out.join(&rel);
        fs::create_dir_all(&dest).at(&dest)?;
    } else if mode.is_some_and(|m| m & 0o170000 == 0o120000) {
        let mut target = String::new();
        data.read_to_string(&mut target).map_err(bad)?;
        write_link(out, &rel, &target)?;
    } else {
        write_file(out, &rel, data, mode)?;
    }
    Ok(())
}

fn untar(reader: impl io::Read, out: &Path) -> Result<()> {
    let mut archive = tar::Archive::new(reader);
    for entry in archive.entries().map_err(bad)? {
        let mut entry = entry.map_err(bad)?;
        let raw = entry.path().map_err(bad)?.to_string_lossy().into_owned();
        let kind = entry.header().entry_type();
        if kind.is_pax_global_extensions()
            || kind.is_pax_local_extensions()
            || kind.is_gnu_longname()
        {
            continue;
        }
        // A bare `./` entry is the archive root.
        let Ok(rel) = safe_relative(&raw) else {
            if raw.trim_start_matches("./").is_empty() {
                continue;
            }
            return Err(InstallError::UnsafePath(raw));
        };
        if kind.is_dir() {
            guard_no_links(out, &rel)?;
            let dest = out.join(&rel);
            fs::create_dir_all(&dest).at(&dest)?;
        } else if kind.is_symlink() {
            let target = entry
                .link_name()
                .map_err(bad)?
                .map(|t| t.to_string_lossy().into_owned())
                .unwrap_or_default();
            write_link(out, &rel, &target)?;
        } else if kind.is_file() {
            let mode = entry.header().mode().ok();
            write_file(out, &rel, &mut entry, mode)?;
        }
        // Hard links, devices and FIFOs have no place in a game build; skip them.
    }
    Ok(())
}

#[cfg(target_os = "macos")]
use std::ffi::OsStr;

#[cfg(target_os = "macos")]
fn undmg(file: &Path, out: &Path) -> Result<()> {
    let mount = out.with_extension("mnt");
    fs::create_dir_all(&mount).at(&mount)?;
    crate::tools::run(
        "hdiutil",
        &[
            OsStr::new("attach"),
            OsStr::new("-nobrowse"),
            OsStr::new("-readonly"),
            OsStr::new("-noautoopen"),
            OsStr::new("-mountpoint"),
            mount.as_os_str(),
            file.as_os_str(),
        ],
        // Images with a license agreement wait for an answer on stdin.
        Some(b"Y\n".as_slice()),
    )?;
    let copied = copy_volume(&mount, out);
    let detached = crate::tools::run(
        "hdiutil",
        &[
            OsStr::new("detach"),
            mount.as_os_str(),
            OsStr::new("-force"),
        ],
        None,
    );
    let _ = fs::remove_dir(&mount);
    copied.and(detached)
}

/// Copies a mounted volume's contents, keeping bundles intact. Hidden entries (Finder layout,
/// Spotlight and Trash folders, which can be unreadable) and shortcuts that point outside the
/// volume, such as the `Applications` link, are left out.
#[cfg(target_os = "macos")]
fn copy_volume(mount: &Path, out: &Path) -> Result<()> {
    for entry in fs::read_dir(mount).at(mount)? {
        let entry = entry.at(mount)?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') || name == "Applications" {
            continue;
        }
        let path = entry.path();
        let dest = out.join(&name);
        if entry.file_type().at(&path)?.is_symlink() {
            let target = fs::read_link(&path).at(&path)?;
            if target.is_relative() && safe_relative(&target.to_string_lossy()).is_ok() {
                std::os::unix::fs::symlink(&target, &dest).at(&dest)?;
            }
            continue;
        }
        crate::tools::run("ditto", &[path.as_os_str(), dest.as_os_str()], None)?;
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn undmg(_file: &Path, _out: &Path) -> Result<()> {
    Err(InstallError::UnsupportedArchive("dmg".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_escaping_names() {
        for bad in ["../x", "/etc/x", "a/../../x", "", "."] {
            assert!(safe_relative(bad).is_err(), "{bad}");
        }
        assert_eq!(safe_relative("./a/b").ok(), Some(PathBuf::from("a/b")));
    }
}
