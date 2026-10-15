//! Keeps the local catalog current: on start, every 15 minutes, and on request.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::error::CommandError;
use crate::state::AppState;

const REFRESH_EVERY: Duration = Duration::from_secs(15 * 60);

/// What the local catalog holds and how its last sync went.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CatalogStatus {
    /// Ports saved on this device.
    pub port_count: u32,
    /// Unix seconds of the last successful check with the server.
    pub last_synced_at: Option<f64>,
    /// Set when the last sync failed (e.g. offline); the saved copy is still used.
    pub last_error: Option<CommandError>,
}

/// Sent after every sync attempt.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct CatalogSynced {
    pub status: CatalogStatus,
    /// True when ports were added, changed or removed.
    pub changed: bool,
}

pub fn status(state: &AppState) -> Result<CatalogStatus, CommandError> {
    let s = state.catalog.state()?;
    Ok(CatalogStatus {
        port_count: u32::try_from(s.port_count).unwrap_or(u32::MAX),
        // f64: TypeScript numbers (seconds fit exactly).
        last_synced_at: s.last_synced_at.map(|t| t as f64),
        last_error: state.last_sync_error(),
    })
}

/// Runs one sync (waiting for a running one to finish first) and tells the UI.
pub async fn sync_now(app: &AppHandle) -> Result<CatalogStatus, CommandError> {
    let state = app.state::<AppState>();
    let changed = {
        let _guard = state.sync_lock.lock().await;
        match portside_catalog::sync(&state.catalog, &state.api).await {
            Ok(outcome) => {
                state.set_last_sync_error(None);
                !matches!(outcome, portside_catalog::SyncOutcome::Unchanged { .. })
            }
            Err(e) => {
                state.set_last_sync_error(Some(e.into()));
                false
            }
        }
    };
    let status = status(&state)?;
    let event = CatalogSynced {
        status: status.clone(),
        changed,
    };
    if let Err(e) = event.emit(app) {
        eprintln!("could not notify the UI of a catalog sync: {e}");
    }
    Ok(status)
}

/// Starts the periodic refresh. Runs for the life of the app.
pub fn spawn_refresh(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            // Errors are recorded in the status; the loop keeps going.
            let _ = sync_now(&app).await;
            tokio::time::sleep(REFRESH_EVERY).await;
        }
    });
}
