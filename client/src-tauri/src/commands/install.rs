use portside_installer::{Installed, PlanSummary};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{CommandError, ErrorCode};
use crate::installs::{self, InstallChanged};
use crate::state::AppState;
use crate::storage::{self, Storage, StorageKind};

/// What installing a port would download, for the confirm step.
#[tauri::command]
#[specta::specta]
pub async fn install_plan(
    state: State<'_, AppState>,
    port_id: String,
) -> Result<PlanSummary, CommandError> {
    let port = state.api.port(&port_id).await?;
    let plan = state.installs.installer().plan(&port).await?;
    if plan.summary.fit == portside_installer::BuildFit::Proton
        && portside_steam::find_proton().is_none()
    {
        return Err(crate::proton::missing());
    }
    let summary = plan.summary.clone();
    state.installs.remember_plan(plan);
    Ok(summary)
}

/// Starts installing in the background; progress arrives as `install-changed` events.
/// `accept_unsigned` is the user's OK for a macOS app Apple hasn't checked. `to` picks the
/// storage for a new install (internal when left out); updates stay where the game is.
#[tauri::command]
#[specta::specta]
pub async fn install_start(
    app: AppHandle,
    port_id: String,
    accept_unsigned: bool,
    to: Option<StorageKind>,
) -> Result<(), CommandError> {
    let to = storage::library_for(to.unwrap_or(StorageKind::Internal))?;
    installs::start(&app, port_id, accept_unsigned, to).await
}

/// Where games can be installed: internal storage, plus the SD card on a Steam Deck with one
/// in.
#[tauri::command]
#[specta::specta]
pub async fn install_storage(state: State<'_, AppState>) -> Result<Vec<Storage>, CommandError> {
    let internal = state.installs.installer().library_dir().to_path_buf();
    tokio::task::spawn_blocking(move || storage::list(&internal))
        .await
        .map_err(|e| CommandError {
            code: ErrorCode::Library,
            message: format!("Couldn't check storage: {e}"),
        })
}

/// Stops a running install. Downloaded bytes are kept, so starting again resumes.
#[tauri::command]
#[specta::specta]
pub fn install_cancel(state: State<'_, AppState>, port_id: String) -> bool {
    state.installs.cancel(&port_id)
}

/// Installs running now.
#[tauri::command]
#[specta::specta]
pub fn installs_active(state: State<'_, AppState>) -> Vec<InstallChanged> {
    state.installs.active()
}

/// Everything installed, by title. On Android, games the player removed in Android's
/// settings are forgotten first.
#[tauri::command]
#[specta::specta]
pub async fn library_list(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Installed>, CommandError> {
    #[cfg(target_os = "android")]
    crate::android::forget_removed(&app).await;
    #[cfg(not(target_os = "android"))]
    let _ = app;
    Ok(state.installs.installer().library().list()?)
}

/// The installed version of one port, if any.
#[tauri::command]
#[specta::specta]
pub fn library_get(
    state: State<'_, AppState>,
    port_id: String,
) -> Result<Option<Installed>, CommandError> {
    Ok(state.installs.installer().library().current(&port_id)?)
}

/// Deletes a port's installed files. Saves stored outside the install folder stay. On
/// Android, Android asks the player to confirm removing the game first.
#[tauri::command]
#[specta::specta]
pub async fn uninstall(
    app: AppHandle,
    state: State<'_, AppState>,
    port_id: String,
) -> Result<(), CommandError> {
    if state.installs.is_running(&port_id) {
        return Err(CommandError {
            code: ErrorCode::InstallRunning,
            message: "Cancel the install first.".to_owned(),
        });
    }
    state.games.check_not_running(&port_id)?;
    #[cfg(target_os = "android")]
    crate::android::before_uninstall(&app, &port_id).await?;
    state.installs.installer().uninstall(&port_id).await?;
    installs::announce_uninstall(&app, &port_id);
    Ok(())
}

/// Shows the install folder in Finder, Explorer or the file manager.
#[tauri::command]
#[specta::specta]
pub fn install_reveal(
    app: AppHandle,
    state: State<'_, AppState>,
    port_id: String,
) -> Result<(), CommandError> {
    let installed = state
        .installs
        .installer()
        .library()
        .current(&port_id)?
        .ok_or_else(|| {
            CommandError::from(portside_installer::InstallError::NotInstalled(
                port_id.clone(),
            ))
        })?;
    app.opener()
        .reveal_item_in_dir(&installed.dir)
        .map_err(|e| CommandError {
            code: ErrorCode::Library,
            message: format!("Couldn't open the folder: {e}"),
        })
}
