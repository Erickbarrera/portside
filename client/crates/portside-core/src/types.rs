//! Enums from the catalog contract (`docs/specs/catalog.md`, `catalog/src/schema/enums.ts`).
//! Wire names are snake_case except `PortType`, which keeps upstream's kebab-case.

use serde::{Deserialize, Serialize};

macro_rules! wire_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[cfg_attr(feature = "specta", derive(specta::Type))]
        pub enum $name {
            $(#[serde(rename = $wire)] $variant),+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// The name used on the wire and in URLs.
            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $wire),+ }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

wire_enum!(
    /// Operating system a build targets.
    Os { Windows = "windows", Macos = "macos", Linux = "linux", Android = "android" }
);

wire_enum!(
    /// CPU architecture a build targets.
    Arch { X86_64 = "x86_64", Arm64 = "arm64", Universal = "universal" }
);

wire_enum!(
    /// Playable status, in order from best to worst (docs/specs/catalog.md).
    PlayableStatus {
        Perfect = "perfect",
        Playable = "playable",
        InProgress = "in_progress",
        Boots = "boots",
        NotPlayable = "not_playable",
    }
);

wire_enum!(
    /// Upstream port type.
    PortType {
        StaticRecomp = "static-recomp",
        SourcePort = "source-port",
        EngineRemake = "engine-remake",
        BrowserPort = "browser-port",
        Decompilation = "decompilation",
        Disassembly = "disassembly",
        Tool = "tool",
    }
);

wire_enum!(
    /// Where a per-OS status came from.
    StatusSource { Curator = "curator", Manifest = "manifest", Seed = "seed" }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_names_round_trip() {
        for s in PlayableStatus::ALL {
            let json = serde_json::to_string(s).expect("serialize");
            assert_eq!(json, format!("\"{}\"", s.as_str()));
            let back: PlayableStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(&back, s);
        }
        assert_eq!(PortType::StaticRecomp.as_str(), "static-recomp");
        assert_eq!(Os::Macos.to_string(), "macos");
    }
}
