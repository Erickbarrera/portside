use portside_installer::PlayStats;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{CommandError, ErrorCode};
use crate::games::{self, GameChanged};
use crate::saves;
use crate::state::AppState;

/// Starts the installed version of a game; `game-changed` events follow it until it exits.
/// Cloud saves sync first unless `skip_save_sync` (fails with `E_SAVES_CONFLICT` when the
/// player has to choose which saves to keep) and again after it exits.
/// Async so it runs inside the runtime the game's process is watched from.
#[tauri::command]
#[specta::specta]
pub async fn game_launch(
    app: AppHandle,
    port_id: String,
    skip_save_sync: bool,
) -> Result<(), CommandError> {
    app.state::<AppState>().games.check_not_running(&port_id)?;
    if !skip_save_sync {
        saves::before_launch(&app, &port_id).await?;
    }
    games::launch(&app, port_id)
}

/// Asks a running game to quit, and ends it if it doesn't within a few seconds.
#[tauri::command]
#[specta::specta]
pub fn game_stop(app: AppHandle, port_id: String) -> bool {
    games::stop(&app, &port_id)
}

/// Games running now.
#[tauri::command]
#[specta::specta]
pub fn games_running(state: State<'_, AppState>) -> Vec<GameChanged> {
    state.games.running()
}

/// Play time for every game ever played from Portside, on this device and, with cloud
/// saves on, on the player's other devices.
#[tauri::command]
#[specta::specta]
pub fn play_stats(state: State<'_, AppState>) -> Result<Vec<PlayStats>, CommandError> {
    let local = state.installs.installer().library().play_stats()?;
    Ok(state.playtime.all(local))
}

/// Shows the output of a game's last run in Finder, Explorer or the file manager.
#[tauri::command]
#[specta::specta]
pub fn game_reveal_log(
    app: AppHandle,
    state: State<'_, AppState>,
    port_id: String,
) -> Result<(), CommandError> {
    let log = state.games.log_for(&port_id);
    if !log.is_file() {
        return Err(CommandError {
            code: ErrorCode::NotFound,
            message: "This game hasn't been started from Portside yet.".to_owned(),
        });
    }
    app.opener()
        .reveal_item_in_dir(&log)
        .map_err(|e| CommandError {
            code: ErrorCode::Library,
            message: format!("Couldn't open the folder: {e}"),
        })
}
