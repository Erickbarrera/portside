use portside_api::SearchParams;
use portside_api::models::{Page, PortDetail, PortSummary};
use portside_core::{Os, PlayableStatus, PortType};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, State};

use crate::catalog_sync::{self, CatalogStatus};
use crate::error::CommandError;
use crate::state::AppState;

/// Catalog filters from the Discover screen.
#[derive(Debug, Clone, Default, Deserialize, Type)]
#[serde(default)]
pub struct CatalogQuery {
    #[specta(optional)]
    pub q: Option<String>,
    #[specta(optional)]
    pub os: Option<Os>,
    #[specta(optional)]
    pub status: Option<PlayableStatus>,
    #[specta(optional)]
    pub port_type: Option<PortType>,
    #[specta(optional)]
    pub cursor: Option<String>,
    #[specta(optional)]
    pub limit: Option<u32>,
}

impl From<CatalogQuery> for SearchParams {
    fn from(q: CatalogQuery) -> Self {
        SearchParams {
            q: q.q,
            os: q.os,
            status: q.status,
            port_type: q.port_type,
            cursor: q.cursor,
            limit: q.limit,
        }
    }
}

/// Searches the catalog saved on this device (works offline). Before the first sync it
/// downloads the catalog.
#[tauri::command]
#[specta::specta]
pub async fn catalog_search(
    app: AppHandle,
    state: State<'_, AppState>,
    query: CatalogQuery,
) -> Result<Page<PortSummary>, CommandError> {
    let params: SearchParams = query.into();
    if state.catalog.state()?.rev.is_none() {
        catalog_sync::sync_now(&app).await?;
        if state.catalog.state()?.rev.is_none() {
            // Never synced and the sync failed: report why.
            return Err(state.last_sync_error().unwrap_or_else(|| {
                portside_api::ApiError::Decode("catalog not downloaded".into()).into()
            }));
        }
    }
    Ok(state.catalog.search(&params)?)
}

/// What the game page shows.
#[derive(Debug, Clone, Serialize, Type)]
pub struct PortView {
    pub port: PortDetail,
    /// True when the catalog was unreachable and `port` holds only the saved summary
    /// (no description, manifest or releases).
    pub offline: bool,
}

/// One port with its manifest and latest releases; the saved summary when offline.
#[tauri::command]
#[specta::specta]
pub async fn port_get(state: State<'_, AppState>, id: String) -> Result<PortView, CommandError> {
    match state.api.port(&id).await {
        Ok(port) => Ok(PortView {
            port,
            offline: false,
        }),
        Err(e @ portside_api::ApiError::Network(_)) => match state.catalog.get(&id)? {
            Some(summary) => Ok(PortView {
                port: summary_only(summary),
                offline: true,
            }),
            None => Err(e.into()),
        },
        Err(e) => Err(e.into()),
    }
}

/// Saved summaries for these ports, in the same order; ports the saved catalog doesn't
/// have are left out. Local only, so the Library shows covers offline.
#[tauri::command]
#[specta::specta]
pub fn catalog_summaries(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<Vec<PortSummary>, CommandError> {
    let mut out = Vec::with_capacity(ids.len());
    for id in &ids {
        if let Some(summary) = state.catalog.get(id)? {
            out.push(summary);
        }
    }
    Ok(out)
}

/// How fresh the saved catalog is.
#[tauri::command]
#[specta::specta]
pub fn catalog_status(state: State<'_, AppState>) -> Result<CatalogStatus, CommandError> {
    catalog_sync::status(&state)
}

/// Checks the server for catalog changes now.
#[tauri::command]
#[specta::specta]
pub async fn catalog_refresh(app: AppHandle) -> Result<CatalogStatus, CommandError> {
    catalog_sync::sync_now(&app).await
}

fn summary_only(summary: PortSummary) -> PortDetail {
    PortDetail {
        summary,
        description: None,
        repo: None,
        homepage: None,
        license: None,
        last_commit: None,
        upstream_score: None,
        ratings: None,
        video_url: None,
        note: None,
        status_notes: None,
        manifest: None,
        manifest_rev: None,
        verified: false,
        latest_releases: Vec::new(),
    }
}
