use std::time::{SystemTime, UNIX_EPOCH};

use portside_api::{Client, SnapshotResult};

use crate::error::CatalogError;
use crate::store::Catalog;

/// What a sync did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    /// Downloaded the catalog and replaced the local copy.
    Updated { rev: u64, ports: usize },
    /// Already current.
    Unchanged { rev: u64 },
}

/// Brings the local copy up to date. The catalog is one static file
/// (docs/specs/static-catalog.md): the request carries the ETag of the copy, so an unchanged
/// catalog costs a 304 and a changed one is downloaded whole.
pub async fn sync(catalog: &Catalog, api: &Client) -> Result<SyncOutcome, CatalogError> {
    let state = catalog.state()?;
    // Without a copy, ask unconditionally (a stale ETag must not leave the copy empty).
    let etag = state.rev.and(state.etag.as_deref());
    match api.snapshot(etag).await? {
        SnapshotResult::NotModified { .. } => {
            catalog.mark_checked(now())?;
            Ok(SyncOutcome::Unchanged {
                rev: state.rev.unwrap_or(0),
            })
        }
        // Same content under a new ETag (e.g. the host re-deployed): nothing to apply.
        SnapshotResult::Snapshot { snapshot, etag } if Some(snapshot.rev) == state.rev => {
            catalog.mark_checked_with_etag(etag.as_deref(), now())?;
            Ok(SyncOutcome::Unchanged { rev: snapshot.rev })
        }
        SnapshotResult::Snapshot { snapshot, etag } => {
            catalog.replace_all(&snapshot, etag.as_deref(), now())?;
            Ok(SyncOutcome::Updated {
                rev: snapshot.rev,
                ports: snapshot.ports.len(),
            })
        }
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
