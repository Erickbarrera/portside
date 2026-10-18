use portside_core::{Arch, Os};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};

use crate::app_update::{self, AppUpdateStatus, AppUpdater};
use crate::error::CommandError;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Type)]
pub struct AppInfo {
    pub version: String,
    pub os: Os,
    pub arch: Arch,
    /// Where the catalog is read from.
    pub catalog_url: String,
    /// Running on a Steam-run TV-style screen (Game Mode, Big Picture): couch mode starts on.
    pub big_screen: bool,
    /// A Steam Deck: text fields open Steam's keyboard when a controller selects them.
    pub steam_deck: bool,
}

/// Version, platform and catalog address, for the UI and Settings.
#[tauri::command]
#[specta::specta]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        os: state.platform.os,
        arch: state.platform.arch,
        catalog_url: state.api.base_url().to_owned(),
        big_screen: crate::steam::big_screen(),
        steam_deck: portside_core::paths::is_steam_deck(),
    }
}

/// Whether Portside can update itself and what the last check found.
#[tauri::command]
#[specta::specta]
pub fn app_update_status(updater: State<'_, AppUpdater>) -> AppUpdateStatus {
    updater.status()
}

/// Checks GitHub for a newer Portside now.
#[tauri::command]
#[specta::specta]
pub async fn app_update_check(app: AppHandle) -> AppUpdateStatus {
    app_update::check(&app).await
}

/// Downloads and installs the update found by the last check, then restarts Portside.
#[tauri::command]
#[specta::specta]
pub async fn app_update_install(app: AppHandle) -> Result<(), CommandError> {
    app_update::install(&app).await
}
