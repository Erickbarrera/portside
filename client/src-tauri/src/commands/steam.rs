use tauri::{AppHandle, State};

use crate::error::CommandError;
use crate::steam::{self, PlayRequest, SteamStatus};

/// Whether Steam is here and has this game, for the game page's "Add to Steam".
#[tauri::command]
#[specta::specta]
pub async fn steam_status(port_id: String) -> SteamStatus {
    steam::status(&port_id).await
}

/// Adds an installed game to Steam's library. Fails with `E_STEAM_RUNNING` while Steam is
/// open unless `restart_steam` (Steam closes, the game is added, Steam opens again).
#[tauri::command]
#[specta::specta]
pub async fn steam_add(
    app: AppHandle,
    port_id: String,
    restart_steam: bool,
) -> Result<(), CommandError> {
    steam::add(&app, &port_id, restart_steam).await
}

/// Shows Steam's on-screen keyboard over Portside (Steam Deck), typing into the focused
/// field. False when Steam isn't running, so the UI uses its own keyboard instead.
#[tauri::command]
#[specta::specta]
pub async fn steam_keyboard(app: AppHandle) -> bool {
    steam::show_keyboard(&app).await
}

/// The game Steam asked Portside to start (`--play <port>`), once; None afterwards.
#[tauri::command]
#[specta::specta]
pub fn play_request(request: State<'_, PlayRequest>) -> Option<String> {
    request.take()
}
