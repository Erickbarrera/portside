//! The player's own copy of a game, for ports that run on its data files
//! (docs/specs/manifest-format.md, "game_files"). Finds copies that Steam, GOG, Epic or the
//! itch app installed (`stores.rs`), and copies the files a port needs into place (`files.rs`).
//! Nothing here downloads anything, and no paths or file names are logged.

mod files;
mod stores;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use files::{copy_files, data_root, has_files};
pub use stores::find;

#[derive(Debug, thiserror::Error)]
pub enum AssetsError {
    #[error("The game's files aren't in that folder.")]
    NotFound,
    #[error("Couldn't copy the game's files: {0}")]
    Io(#[from] std::io::Error),
}

/// Where a copy of the game came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum Store {
    Steam,
    Gog,
    Epic,
    Itch,
    /// A folder the player chose.
    Folder,
}

impl Store {
    pub fn as_str(self) -> &'static str {
        match self {
            Store::Steam => "steam",
            Store::Gog => "gog",
            Store::Epic => "epic",
            Store::Itch => "itch",
            Store::Folder => "folder",
        }
    }

    pub fn parse(s: &str) -> Option<Store> {
        [
            Store::Steam,
            Store::Gog,
            Store::Epic,
            Store::Itch,
            Store::Folder,
        ]
        .into_iter()
        .find(|st| st.as_str() == s)
    }
}

/// A copy of the game on this computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Found {
    pub store: Store,
    /// The folder holding the files the port needs (inside the store's game folder).
    pub dir: PathBuf,
}
