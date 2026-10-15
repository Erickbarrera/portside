use portside_core::{Arch, Os};
use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Type)]
pub struct AppInfo {
    pub version: String,
    pub os: Os,
    pub arch: Arch,
    /// Where the catalog is read from.
    pub catalog_url: String,
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
    }
}
