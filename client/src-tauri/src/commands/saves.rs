//! Cloud saves (docs/specs/save-sync-protocol.md).

use std::path::PathBuf;

use portside_sync::DeviceRecord;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::error::{CommandError, ErrorCode};
use crate::saves::{self, GameSaves, SavesStatus};

fn folder(path: &str) -> Result<PathBuf, CommandError> {
    let path = PathBuf::from(path.trim());
    if !path.is_absolute() || !path.is_dir() {
        return Err(CommandError {
            code: ErrorCode::Invalid,
            message: "Choose a folder that exists on this computer.".to_owned(),
        });
    }
    Ok(path)
}

/// Whether cloud saves are on, where, and synced folders found on this computer.
#[tauri::command]
#[specta::specta]
pub async fn saves_status(app: AppHandle) -> Result<SavesStatus, CommandError> {
    saves::blocking(&app, |s| Ok(s.status())).await
}

/// Opens the system folder picker. None when the player cancels.
#[tauri::command]
#[specta::specta]
pub async fn saves_pick_folder(app: AppHandle, title: String) -> Option<String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title(title)
        .pick_folder(move |picked| {
            let _ = tx.send(picked.and_then(|p| p.into_path().ok()));
        });
    rx.await.ok().flatten().map(|p| p.display().to_string())
}

/// Turns cloud saves on with the Portside saves in `path`, or new ones if it has none.
#[tauri::command]
#[specta::specta]
pub async fn saves_use_folder(app: AppHandle, path: String) -> Result<SavesStatus, CommandError> {
    let path = folder(&path)?;
    saves::blocking(&app, move |s| {
        s.use_folder(&path)?;
        Ok(s.status())
    })
    .await
}

/// Stops syncing on this device. Saves on disk and in the folder are left alone.
#[tauri::command]
#[specta::specta]
pub async fn saves_turn_off(app: AppHandle) -> Result<SavesStatus, CommandError> {
    saves::blocking(&app, |s| {
        s.turn_off()?;
        Ok(s.status())
    })
    .await
}

/// Devices that use the same cloud saves, most recently synced first.
#[tauri::command]
#[specta::specta]
pub async fn saves_devices(app: AppHandle) -> Result<Vec<DeviceRecord>, CommandError> {
    saves::blocking(&app, |s| s.devices()).await
}

/// A game's cloud saves as last known.
#[tauri::command]
#[specta::specta]
pub async fn saves_game(app: AppHandle, port_id: String) -> Result<GameSaves, CommandError> {
    saves::game(&app, &port_id).await
}

/// Syncs a game's saves now.
#[tauri::command]
#[specta::specta]
pub async fn saves_sync(app: AppHandle, port_id: String) -> Result<GameSaves, CommandError> {
    saves::sync(&app, &port_id).await
}

/// Settles a conflict: every device gets `snapshot_id`'s saves. The other side stays in the
/// history.
#[tauri::command]
#[specta::specta]
pub async fn saves_keep(
    app: AppHandle,
    port_id: String,
    snapshot_id: String,
) -> Result<GameSaves, CommandError> {
    saves::keep(&app, &port_id, snapshot_id).await
}

/// Sets the folder a game keeps its saves in on this device; None goes back to the game's
/// listing.
#[tauri::command]
#[specta::specta]
pub async fn saves_set_game_folder(
    app: AppHandle,
    port_id: String,
    path: Option<String>,
) -> Result<GameSaves, CommandError> {
    let path = path.as_deref().map(folder).transpose()?;
    let id = port_id.clone();
    saves::blocking(&app, move |s| {
        s.set_custom_folder(&id, path.map(|p| p.display().to_string()).as_deref())
    })
    .await?;
    saves::game(&app, &port_id).await
}

/// Shows a game's save folder in Finder, Explorer or the file manager.
#[tauri::command]
#[specta::specta]
pub async fn saves_reveal(app: AppHandle, port_id: String) -> Result<(), CommandError> {
    let game = saves::game(&app, &port_id).await?;
    let Some(dir) = game.folder.filter(|f| std::path::Path::new(f).is_dir()) else {
        return Err(CommandError {
            code: ErrorCode::NotFound,
            message: "This game hasn't made its save folder yet.".to_owned(),
        });
    };
    app.opener()
        .open_path(dir, None::<&str>)
        .map_err(|e| CommandError {
            code: ErrorCode::Saves,
            message: format!("Couldn't open the folder: {e}"),
        })
}
