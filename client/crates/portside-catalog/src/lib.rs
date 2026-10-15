//! Local copy of the Portside catalog in SQLite, kept current from the static catalog's
//! `v1/snapshot.json` (downloaded whole when its ETag changes).
//!
//! The UI searches this copy, so Discover works offline. Full port details (description,
//! manifest, releases) come from `v1/ports/{id}.json`.

mod error;
mod search;
mod store;
mod sync;

pub use error::CatalogError;
pub use store::{CacheState, Catalog};
pub use sync::{SyncOutcome, sync};
