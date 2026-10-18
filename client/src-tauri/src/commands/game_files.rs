//! The player's own game files (docs/specs/manifest-format.md, "game_files").

use std::path::PathBuf;

use portside_assets::Store;
use tauri::AppHandle;

use crate::error::{CommandError, ErrorCode};
use crate::game_files::{self, GameFilesStatus};

/// Whether a port needs files from the player's copy of the game, whether the installed
/// version has them, and copies of the game found on this computer. None when it needs none.
#[tauri::command]
#[specta::specta]
pub async fn game_files_status(
    app: AppHandle,
    port_id: String,
) -> Result<Option<GameFilesStatus>, CommandError> {
    game_files::status(&app, &port_id).await
}

/// Copies the files from `dir` into place for the installed version: a copy
/// `game_files_status` found (with its `store`), or a folder the player chose (`folder`).
#[tauri::command]
#[specta::specta]
pub async fn game_files_copy(
    app: AppHandle,
    port_id: String,
    store: Store,
    dir: String,
) -> Result<Option<GameFilesStatus>, CommandError> {
    let dir = PathBuf::from(dir.trim());
    if !dir.is_absolute() || !dir.is_dir() {
        return Err(CommandError {
            code: ErrorCode::Invalid,
            message: "Choose a folder that exists on this computer.".to_owned(),
        });
    }
    game_files::copy_from(&app, &port_id, store, dir).await
}
