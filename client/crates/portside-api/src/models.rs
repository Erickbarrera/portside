//! Wire types of the static catalog's `v1/` files. Field-level contract:
//! `catalog/src/schema/catalog.ts` and `manifest.ts`. Required-but-nullable fields are `Option<T>` that serialize as `null`;
//! optional fields are `Option<T>` with `skip_serializing_if`, so a value round-trips to the
//! same JSON (the contract test relies on that).
//!
//! Timestamps stay RFC 3339 strings; the UI formats them.

use std::collections::BTreeMap;

use portside_core::{Arch, Os, PlayableStatus, PortType, StatusSource};
use serde::{Deserialize, Serialize};

#[cfg(feature = "specta")]
use specta::Type;

macro_rules! model {
    ($item:item) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[cfg_attr(feature = "specta", derive(Type))]
        $item
    };
}

model! {
    /// `{ items, cursor }`; `cursor: null` means the last page.
    pub struct Page<T> {
        pub items: Vec<T>,
        pub cursor: Option<String>,
    }
}

model! {
    pub struct OsStatus {
        pub os: Os,
        pub status: PlayableStatus,
        pub source: StatusSource,
    }
}

model! {
    pub struct PortSummary {
        pub id: String,
        pub game_key: String,
        pub title: String,
        #[serde(rename = "type")]
        pub port_type: PortType,
        pub status: PlayableStatus,
        pub os_status: Vec<OsStatus>,
        pub stars: Option<i64>,
        pub archived: bool,
        pub screenshot_url: Option<String>,
        pub play_url: Option<String>,
        pub updated_at: String,
    }
}

model! {
    /// Upstream 1–5 ratings.
    pub struct Ratings {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub playability: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub completeness: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub activity: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub popularity: Option<u8>,
    }
}

model! {
    pub struct ReleaseAsset {
        pub os: Os,
        pub arch: Arch,
        pub name: String,
        pub url: String,
        pub size: u64,
        pub sha256: Option<String>,
    }
}

model! {
    pub struct Release {
        pub id: String,
        pub port_id: String,
        pub tag: String,
        pub name: Option<String>,
        pub published_at: String,
        pub prerelease: bool,
        pub revoked: bool,
        pub html_url: String,
        pub assets: Vec<ReleaseAsset>,
    }
}

model! {
    pub struct PortDetail {
        #[serde(flatten)]
        pub summary: PortSummary,
        pub description: Option<String>,
        pub repo: Option<String>,
        pub homepage: Option<String>,
        pub license: Option<String>,
        pub last_commit: Option<String>,
        pub upstream_score: Option<f64>,
        pub ratings: Option<Ratings>,
        pub video_url: Option<String>,
        pub note: Option<String>,
        pub status_notes: Option<String>,
        pub manifest: Option<Manifest>,
        pub manifest_rev: Option<String>,
        pub verified: bool,
        pub latest_releases: Vec<Release>,
    }
}

model! {
    pub struct Game {
        pub game_key: String,
        pub title: String,
        pub original_platforms: Vec<String>,
        pub ports: Vec<PortSummary>,
    }
}

model! {
    pub struct CatalogSnapshot {
        pub rev: u64,
        pub ports: Vec<PortSummary>,
    }
}

// ---- Manifest, schema 1 (docs/specs/manifest-format.md) ----

model! {
    pub struct Manifest {
        /// Always 1.
        pub schema: u32,
        pub id: String,
        pub game_key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        pub status: PlayableStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_notes: Option<String>,
        pub source: ManifestSource,
        pub builds: Vec<ManifestBuild>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub assets_required: Option<Vec<AssetRequirement>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub saves: Option<ManifestSaves>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub multiplayer: Option<Multiplayer>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub online: Option<Online>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub redistribution: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub maintainers_verified: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub links: Option<BTreeMap<String, Option<String>>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub changelog: Option<Vec<String>>,
    }
}

model! {
    pub struct ManifestSource {
        #[serde(rename = "type")]
        pub source_type: SourceType,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub repo: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub channel: Option<ReleaseChannel>,
        /// Optional and nullable: absent, `null` and a pattern are all valid.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            with = "nullable"
        )]
        #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
        pub tag_pattern: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub min_version: Option<String>,
    }
}

model! {
    pub struct ManifestBuild {
        pub os: Os,
        pub arch: Arch,
        /// Glob matched against release asset names.
        pub asset: String,
        pub archive: ArchiveKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub strip_root: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub launch: Option<LaunchSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub checksum: Option<ChecksumSource>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub unsigned: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub android_package: Option<String>,
    }
}

model! {
    pub struct LaunchSpec {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub exe: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub app_bundle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub args: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub cwd: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub env: Option<BTreeMap<String, String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub chmod_x: Option<bool>,
    }
}

model! {
    pub struct AssetRequirement {
        pub id: String,
        pub label: String,
        pub accept: Vec<AssetAccept>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub formats: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "nullable")]
        #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
        pub normalize: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub import: Option<AssetImport>,
    }
}

model! {
    /// Metadata of a known-good dump. Hashes only; never the file (docs/legal/policy.md).
    pub struct AssetAccept {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sha1: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sha256: Option<String>,
        pub name: String,
    }
}

model! {
    pub struct AssetImport {
        pub mode: ImportMode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub dest: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "nullable")]
        #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
        pub run: Option<Option<String>>,
    }
}

model! {
    pub struct ManifestSaves {
        pub root: SaveRoots,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub include: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub exclude: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub portable: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub tied_to_version: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "nullable")]
        #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
        pub converter: Option<Option<String>>,
    }
}

model! {
    pub struct SaveRoots {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub windows: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub macos: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub linux: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub android: Option<String>,
    }
}

model! {
    pub struct Online {
        #[serde(default, skip_serializing_if = "Option::is_none", with = "nullable")]
        #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
        pub join_uri: Option<Option<String>>,
    }
}

macro_rules! simple_enum {
    ($name:ident { $($variant:ident = $wire:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "specta", derive(Type))]
        pub enum $name { $(#[serde(rename = $wire)] $variant),+ }

        impl $name {
            /// The name used in manifests.
            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $wire),+ }
            }
        }
    };
}

simple_enum!(SourceType {
    GithubReleases = "github_releases",
    GithubActionsArtifact = "github_actions_artifact",
    UrlList = "url_list",
});
simple_enum!(ReleaseChannel {
    Stable = "stable",
    Prerelease = "prerelease",
    TagPattern = "tag_pattern",
});
simple_enum!(ArchiveKind {
    Zip = "zip",
    TarGz = "tar.gz",
    TarXz = "tar.xz",
    SevenZ = "7z",
    None = "none",
    Apk = "apk",
    Dmg = "dmg",
    AppImage = "appimage",
});
simple_enum!(ChecksumSource {
    Upstream = "upstream",
    Attestation = "attestation",
    None = "none",
});
simple_enum!(ImportMode {
    Copy = "copy",
    ExtractWithPort = "extract_with_port",
    PickFolder = "pick_folder",
});
simple_enum!(Multiplayer {
    None = "none",
    Local = "local",
    OnlineCoop = "online_coop",
    OnlinePvp = "online_pvp",
});

/// Optional *and* nullable fields: `None` = absent, `Some(None)` = `null`.
mod nullable {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer, T: Serialize>(
        value: &Option<Option<T>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(inner) => inner.serialize(s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
        d: D,
    ) -> Result<Option<Option<T>>, D::Error> {
        Option::<T>::deserialize(d).map(Some)
    }
}
