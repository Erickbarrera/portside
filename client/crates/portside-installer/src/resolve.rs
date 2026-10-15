//! Which build fits this machine, and which release file it is.

use globset::Glob;
use portside_api::models::{ManifestBuild, Release};
use portside_core::Platform;
use portside_core::types::{Arch, Os};
use serde::{Deserialize, Serialize};

use crate::error::{InstallError, Result};

/// How a build fits the machine (client/docs/install-launch.md, "Pick build").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum BuildFit {
    /// Built for this OS and architecture.
    Native,
    /// One build for every architecture of this OS.
    Universal,
    /// An x86_64 build run through translation (Rosetta on macOS, emulation on Windows).
    Translated,
}

/// The best build for `platform`: exact match, then universal, then x86_64 under
/// translation on arm64 macOS or Windows.
pub fn pick_build(
    builds: &[ManifestBuild],
    platform: Platform,
) -> Option<(&ManifestBuild, BuildFit)> {
    let for_os = || builds.iter().filter(move |b| b.os == platform.os);
    if let Some(b) = for_os().find(|b| b.arch == platform.arch) {
        return Some((b, BuildFit::Native));
    }
    if let Some(b) = for_os().find(|b| b.arch == Arch::Universal) {
        return Some((b, BuildFit::Universal));
    }
    let translates = platform.arch == Arch::Arm64 && matches!(platform.os, Os::Macos | Os::Windows);
    if translates && let Some(b) = for_os().find(|b| b.arch == Arch::X86_64) {
        return Some((b, BuildFit::Translated));
    }
    None
}

/// One downloadable file of a release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseFile {
    pub name: String,
    pub url: String,
    pub size: u64,
    /// Lowercase hex SHA-256 when the source publishes one (Portside API, GitHub `digest`).
    pub sha256: Option<String>,
}

/// A release as the installer needs it, from the Portside API or from GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpstreamRelease {
    pub tag: String,
    pub html_url: String,
    pub files: Vec<ReleaseFile>,
}

impl From<&Release> for UpstreamRelease {
    fn from(r: &Release) -> Self {
        UpstreamRelease {
            tag: r.tag.clone(),
            html_url: r.html_url.clone(),
            files: r
                .assets
                .iter()
                .map(|a| ReleaseFile {
                    name: a.name.clone(),
                    url: a.url.clone(),
                    size: a.size,
                    sha256: a.sha256.as_ref().map(|s| s.to_ascii_lowercase()),
                })
                .collect(),
        }
    }
}

impl UpstreamRelease {
    /// The one file whose name matches the manifest's `asset` glob.
    pub fn file_matching(&self, pattern: &str) -> Result<&ReleaseFile> {
        let glob = Glob::new(pattern)
            .map_err(|e| InstallError::BadPattern {
                pattern: pattern.to_owned(),
                reason: e.kind().to_string(),
            })?
            .compile_matcher();
        let mut hits = self.files.iter().filter(|f| glob.is_match(&f.name));
        match (hits.next(), hits.count()) {
            (Some(f), 0) => Ok(f),
            (None, _) => Err(InstallError::AssetMissing {
                tag: self.tag.clone(),
                pattern: pattern.to_owned(),
            }),
            (Some(_), more) => Err(InstallError::AssetAmbiguous {
                tag: self.tag.clone(),
                pattern: pattern.to_owned(),
                count: more + 1,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use portside_api::models::{ArchiveKind, ManifestBuild};

    use super::*;

    fn build(os: Os, arch: Arch) -> ManifestBuild {
        ManifestBuild {
            os,
            arch,
            asset: format!("{}-{}.zip", os.as_str(), arch.as_str()),
            archive: ArchiveKind::Zip,
            nested: None,
            strip_root: None,
            launch: None,
            checksum: None,
            unsigned: None,
            android_package: None,
        }
    }

    const MAC_ARM: Platform = Platform {
        os: Os::Macos,
        arch: Arch::Arm64,
    };

    #[test]
    fn prefers_native_then_universal_then_translated() {
        let all = [
            build(Os::Macos, Arch::X86_64),
            build(Os::Macos, Arch::Universal),
            build(Os::Macos, Arch::Arm64),
        ];
        assert_eq!(
            pick_build(&all, MAC_ARM).map(|(b, f)| (b.arch, f)),
            Some((Arch::Arm64, BuildFit::Native))
        );
        assert_eq!(
            pick_build(&all[..2], MAC_ARM).map(|(b, f)| (b.arch, f)),
            Some((Arch::Universal, BuildFit::Universal))
        );
        assert_eq!(
            pick_build(&all[..1], MAC_ARM).map(|(b, f)| (b.arch, f)),
            Some((Arch::X86_64, BuildFit::Translated))
        );
    }

    #[test]
    fn no_translation_on_linux_or_from_other_os() {
        let linux_arm = Platform {
            os: Os::Linux,
            arch: Arch::Arm64,
        };
        assert!(pick_build(&[build(Os::Linux, Arch::X86_64)], linux_arm).is_none());
        assert!(pick_build(&[build(Os::Windows, Arch::X86_64)], MAC_ARM).is_none());
    }

    fn release(names: &[&str]) -> UpstreamRelease {
        UpstreamRelease {
            tag: "v1".into(),
            html_url: String::new(),
            files: names
                .iter()
                .map(|n| ReleaseFile {
                    name: (*n).into(),
                    url: format!("https://github.com/x/y/releases/download/v1/{n}"),
                    size: 1,
                    sha256: None,
                })
                .collect(),
        }
    }

    #[test]
    fn glob_must_match_exactly_one_file() {
        let r = release(&[
            "Example-1.2-mac.zip",
            "Example-1.2-win.zip",
            "Example-1.2-win.zip.sha256",
        ]);
        assert_eq!(
            r.file_matching("Example-*-mac.zip")
                .map(|f| f.name.as_str())
                .ok(),
            Some("Example-1.2-mac.zip")
        );
        assert!(matches!(
            r.file_matching("Example-*-linux.zip"),
            Err(InstallError::AssetMissing { .. })
        ));
        assert!(matches!(
            r.file_matching("Example-1.2-*"),
            Err(InstallError::AssetAmbiguous { count: 3, .. })
        ));
    }
}
