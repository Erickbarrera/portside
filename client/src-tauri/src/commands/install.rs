use portside_installer::{Installed, PlanSummary};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{CommandError, ErrorCode};
use crate::installs::{self, InstallChanged};
use crate::state::AppState;

/// What installing a port would download, for the confirm step.
#[tauri::command]
#[specta::specta]
pub async fn install_plan(
    state: State<'_, AppState>,
    port_id: String,
) -> Result<PlanSummary, CommandError> {
    let port = state.api.port(&port_id).await?;
    let plan = state.installs.installer().plan(&port).await?;
    let summary = plan.summary.clone();
    state.installs.remember_plan(plan);
    Ok(summary)
}

/// Starts installing in the background; progress arrives as `install-changed` events.
/// `accept_unsigned` is the user's OK for a macOS app Apple hasn't checked.
#[tauri::command]
#[specta::specta]
pub async fn install_start(
    app: AppHandle,
    port_id: String,
    accept_unsigned: bool,
) -> Result<(), CommandError> {
    installs::start(&app, port_id, accept_unsigned).await
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

/// Everything installed, by title.
#[tauri::command]
#[specta::specta]
pub fn library_list(state: State<'_, AppState>) -> Result<Vec<Installed>, CommandError> {
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

/// Deletes a port's installed files. Saves stored outside the install folder stay.
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
