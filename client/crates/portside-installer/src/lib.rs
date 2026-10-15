//! Installs ports from their upstream GitHub releases: picks the build for this machine,
//! downloads it (resuming), checks it against the published checksum, unpacks it into
//! `{library}/{port_id}/{version}/` and records it (client/docs/install-launch.md).
//!
//! No Tauri here; `src-tauri` calls [`Installer`] and turns [`Progress`] into events.

mod download;
mod error;
mod github;
mod library;
mod resolve;
#[cfg(target_os = "macos")]
mod tools;
mod unpack;
mod verify;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use portside_api::models::{ChecksumSource, ManifestBuild, PortDetail, ReleaseChannel, SourceType};
use portside_core::Platform;
use portside_core::types::{Arch, Os};
use regex::Regex;
use serde::Serialize;

pub use crate::download::{Downloader, HostPolicy};
use crate::error::IoContext;
pub use crate::error::{InstallError, Result};
pub use crate::github::{GITHUB_API, Github};
pub use crate::library::{Installed, Library, PlayStats, Verification};
pub use crate::resolve::{BuildFit, ReleaseFile, UpstreamRelease, pick_build};

/// Manifest schema this client understands (docs/specs/manifest-format.md, "Versioning").
pub const MANIFEST_SCHEMA: u32 = 1;

/// Largest checksum file read into memory.
const MAX_SUMS_BYTES: u64 = 1 << 20;

pub struct InstallerConfig {
    pub library_dir: PathBuf,
    pub platform: Platform,
    pub user_agent: String,
    /// `https://api.github.com` outside tests.
    pub github_api: String,
    pub hosts: HostPolicy,
}

/// What an install would fetch, shown before the user confirms.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PlanSummary {
    pub port_id: String,
    pub title: String,
    pub version: String,
    pub release_url: String,
    pub repo: Option<String>,
    pub asset: String,
    /// Download size in bytes.
    pub size: u64,
    pub os: Os,
    pub arch: Arch,
    pub fit: BuildFit,
    /// macOS app not signed by a registered developer: needs the user's OK.
    pub unsigned: bool,
    pub verification: Verification,
    /// The version installed now, if any.
    pub installed_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InstallPlan {
    pub summary: PlanSummary,
    file: ReleaseFile,
    sums: Option<ReleaseFile>,
    build: ManifestBuild,
}

/// Where an install has got to.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum Progress {
    Downloading { done: u64, total: Option<u64> },
    Verifying,
    Unpacking,
}

pub struct Installer {
    library_dir: PathBuf,
    platform: Platform,
    github: Github,
    downloader: Downloader,
    library: Library,
}

impl Installer {
    pub fn new(config: InstallerConfig, library: Library) -> Result<Installer> {
        Ok(Installer {
            github: Github::new(&config.github_api, &config.user_agent)?,
            downloader: Downloader::new(config.hosts, &config.user_agent)?,
            library_dir: config.library_dir,
            platform: config.platform,
            library,
        })
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    /// Works out what installing `port` means on this machine, without downloading it.
    pub async fn plan(&self, port: &PortDetail) -> Result<InstallPlan> {
        let manifest = port.manifest.as_ref().ok_or(InstallError::NoManifest)?;
        if manifest.schema != MANIFEST_SCHEMA {
            return Err(InstallError::UnsupportedSchema(manifest.schema));
        }
        if manifest.source.source_type != SourceType::GithubReleases {
            return Err(InstallError::UnsupportedSource(
                manifest.source.source_type.as_str().to_owned(),
            ));
        }
        let (build, fit) =
            pick_build(&manifest.builds, self.platform).ok_or_else(|| InstallError::NoBuild {
                os: self.platform.os.as_str().to_owned(),
                arch: self.platform.arch.as_str().to_owned(),
            })?;

        let (release, from_portside) = match from_portside(port, &build.asset)? {
            Some(r) => (r, true),
            None => (self.github.latest(&manifest.source).await?, false),
        };
        let file = release.file_matching(&build.asset)?.clone();
        let (verification, sums) = match (&file.sha256, build.checksum) {
            (Some(_), _) if from_portside => (Verification::Portside, None),
            (Some(_), _) => (Verification::Github, None),
            (None, Some(ChecksumSource::None)) => (Verification::None, None),
            (None, checksum) => match verify::sums_file_for(&release, &file.name) {
                Some(s) => (Verification::ChecksumFile, Some(s.clone())),
                None if checksum == Some(ChecksumSource::Upstream) => {
                    return Err(InstallError::ChecksumMissing(file.name.clone()));
                }
                None => (Verification::None, None),
            },
        };
        let title = manifest
            .display_name
            .clone()
            .unwrap_or_else(|| port.summary.title.clone());
        let installed_version = self.library.current(&port.summary.id)?.map(|i| i.version);
        Ok(InstallPlan {
            summary: PlanSummary {
                port_id: port.summary.id.clone(),
                title,
                version: release.tag.clone(),
                release_url: release.html_url.clone(),
                repo: manifest.source.repo.clone(),
                asset: file.name.clone(),
                size: file.size,
                os: build.os,
                arch: build.arch,
                fit,
                unsigned: build.unsigned == Some(true) && build.os == Os::Macos,
                verification,
                installed_version,
            },
            file,
            sums,
            build: build.clone(),
        })
    }

    /// Downloads, checks and unpacks `plan`, then records it as the current version.
    /// `accept_unsigned` is the user's OK for an app Apple hasn't checked.
    pub async fn install(
        &self,
        plan: &InstallPlan,
        accept_unsigned: bool,
        cancel: &Arc<AtomicBool>,
        progress: &(dyn Fn(Progress) + Send + Sync),
    ) -> Result<Installed> {
        let s = &plan.summary;
        if s.unsigned && !accept_unsigned {
            return Err(InstallError::NeedsConsent);
        }
        let port_dir = self.library_dir.join(file_name(&s.port_id, false)?);
        let version_dir = port_dir.join(file_name(&s.version, true)?);
        let part = self.library_dir.join(".partial").join(format!(
            "{}--{}.part",
            file_name(&s.port_id, false)?,
            file_name(&s.asset, true)?
        ));

        let report = |done, total| progress(Progress::Downloading { done, total });
        self.downloader
            .fetch(&plan.file.url, &part, Some(plan.file.size), cancel, &report)
            .await?;

        progress(Progress::Verifying);
        let expected = match (&plan.file.sha256, &plan.sums) {
            (Some(h), _) => Some(h.clone()),
            (None, Some(sums)) => Some(self.sums_hash(sums, &plan.file.name).await?),
            (None, None) => None,
        };
        let actual = blocking({
            let part = part.clone();
            move || verify::sha256_file(&part)
        })
        .await?;
        if let Some(expected) = expected
            && expected != actual
        {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(InstallError::HashMismatch { expected, actual });
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(InstallError::Cancelled);
        }

        progress(Progress::Unpacking);
        let staging = self.library_dir.join(".staging").join(format!(
            "{}-{}",
            file_name(&s.port_id, false)?,
            unix_now()
        ));
        let job = Unpack {
            build: plan.build.clone(),
            asset: plan.file.name.clone(),
            part: part.clone(),
            staging: staging.clone(),
            target: version_dir.clone(),
            unsigned: s.unsigned,
        };
        let unpacked = blocking(move || job.run()).await;
        let _ = std::fs::remove_dir_all(&staging);
        unpacked?;
        let _ = tokio::fs::remove_file(&part).await;
        let pointer = port_dir.join("current");
        tokio::fs::write(&pointer, file_name(&s.version, true)?)
            .await
            .at(&pointer)?;

        let installed = Installed {
            port_id: s.port_id.clone(),
            title: s.title.clone(),
            version: s.version.clone(),
            dir: version_dir.to_string_lossy().into_owned(),
            os: s.os,
            arch: s.arch,
            fit: s.fit,
            asset: s.asset.clone(),
            release_url: s.release_url.clone(),
            size: s.size,
            sha256: Some(actual),
            verified: s.verification,
            installed_at: unix_now(),
            unsigned: s.unsigned,
            build: plan.build.clone(),
        };
        self.library.add(&installed)?;
        Ok(installed)
    }

    /// Removes every installed version of `port_id`. Saves kept elsewhere are untouched.
    pub async fn uninstall(&self, port_id: &str) -> Result<()> {
        let dir = self.library_dir.join(file_name(port_id, false)?);
        if tokio::fs::metadata(&dir).await.is_ok() {
            tokio::fs::remove_dir_all(&dir).await.at(&dir)?;
        }
        if self.library.remove(port_id)? == 0 {
            return Err(InstallError::NotInstalled(port_id.to_owned()));
        }
        Ok(())
    }

    async fn sums_hash(&self, sums: &ReleaseFile, asset: &str) -> Result<String> {
        if sums.size > MAX_SUMS_BYTES {
            return Err(InstallError::ChecksumMissing(asset.to_owned()));
        }
        let dir = self.library_dir.join(".partial");
        let path = dir.join(format!("sums-{}", unix_now()));
        let never = Arc::new(AtomicBool::new(false));
        let fetched = self
            .downloader
            .fetch(&sums.url, &path, Some(sums.size), &never, &|_, _| {})
            .await;
        let text = match fetched {
            Ok(_) => tokio::fs::read_to_string(&path).await.at(&path),
            Err(e) => Err(e),
        };
        let _ = tokio::fs::remove_file(&path).await;
        verify::parse_sums(&text?, asset)
            .ok_or_else(|| InstallError::ChecksumMissing(asset.to_owned()))
    }
}

/// The newest non-revoked release the Portside API lists that has a file for `pattern`.
fn from_portside(port: &PortDetail, pattern: &str) -> Result<Option<UpstreamRelease>> {
    let Some(manifest) = &port.manifest else {
        return Ok(None);
    };
    let channel = manifest.source.channel.unwrap_or(ReleaseChannel::Stable);
    let tag_re = match (channel, manifest.source.tag_pattern.clone().flatten()) {
        (ReleaseChannel::TagPattern, Some(p)) => {
            Some(Regex::new(&p).map_err(|e| InstallError::BadPattern {
                pattern: p,
                reason: e.to_string(),
            })?)
        }
        _ => None,
    };
    Ok(port
        .latest_releases
        .iter()
        .filter(|r| !r.revoked)
        .filter(|r| channel != ReleaseChannel::Stable || !r.prerelease)
        .filter(|r| tag_re.as_ref().is_none_or(|re| re.is_match(&r.tag)))
        .map(UpstreamRelease::from)
        .find(|r| r.file_matching(pattern).is_ok()))
}

/// Unpacking and moving into place; blocking file work.
struct Unpack {
    build: ManifestBuild,
    asset: String,
    part: PathBuf,
    staging: PathBuf,
    target: PathBuf,
    unsigned: bool,
}

impl Unpack {
    fn run(self) -> Result<()> {
        let _ = std::fs::remove_dir_all(&self.staging);
        let root = unpack::unpack(
            self.build.archive,
            &self.part,
            &self.asset,
            &self.staging.join("files"),
            self.build.strip_root == Some(true),
        )?;
        let launch = self.build.launch.as_ref();
        if let Some(bundle) = launch.and_then(|l| l.app_bundle.as_deref()) {
            let path = root.join(bundle);
            if !path.is_dir() {
                return Err(InstallError::LaunchTargetMissing(bundle.to_owned()));
            }
            if self.unsigned {
                clear_quarantine(&path);
            }
        }
        if let Some(exe) = launch.and_then(|l| l.exe.as_deref()) {
            let path = root.join(exe);
            if !path.is_file() {
                return Err(InstallError::LaunchTargetMissing(exe.to_owned()));
            }
            if launch.and_then(|l| l.chmod_x) == Some(true) {
                unpack::make_executable(&path)?;
            }
        }
        if let Some(parent) = self.target.parent() {
            std::fs::create_dir_all(parent).at(parent)?;
        }
        if self.target.exists() {
            std::fs::remove_dir_all(&self.target).at(&self.target)?;
        }
        std::fs::rename(&root, &self.target).at(&self.target)?;
        Ok(())
    }
}

/// Removes macOS's "downloaded from the internet" flag from an app the user agreed to run.
/// Best effort: files Portside unpacks itself usually don't carry the flag.
fn clear_quarantine(bundle: &Path) {
    #[cfg(target_os = "macos")]
    {
        let _ = tools::run(
            "xattr",
            &[
                std::ffi::OsStr::new("-dr"),
                std::ffi::OsStr::new("com.apple.quarantine"),
                bundle.as_os_str(),
            ],
            None,
        );
    }
    #[cfg(not(target_os = "macos"))]
    let _ = bundle;
}

/// A name safe to use as one path component. Port ids must already be safe; release tags
/// and file names have anything unusual replaced.
fn file_name(raw: &str, lenient: bool) -> Result<String> {
    let ok = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+');
    let name: String = if lenient {
        raw.chars().map(|c| if ok(c) { c } else { '_' }).collect()
    } else if raw.chars().all(ok) {
        raw.to_owned()
    } else {
        return Err(InstallError::UnsafePath(raw.to_owned()));
    };
    if name.is_empty() || name.chars().all(|c| c == '.') || name.starts_with('.') {
        return Err(InstallError::UnsafePath(raw.to_owned()));
    }
    Ok(name)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| InstallError::Io {
            path: PathBuf::new(),
            source: std::io::Error::other(e.to_string()),
        })?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_stay_one_component() {
        assert_eq!(
            file_name("v1.2/beta 3", true).ok().as_deref(),
            Some("v1.2_beta_3")
        );
        assert!(file_name("../x", false).is_err());
        assert!(file_name("..", true).is_err());
        assert!(file_name(".hidden", true).is_err());
        assert_eq!(
            file_name("example-port", false).ok().as_deref(),
            Some("example-port")
        );
    }
}
