use thiserror::Error;

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("catalog database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("catalog database migration failed: {0}")]
    Migration(#[from] refinery::Error),
    #[error("stored catalog row is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Api(#[from] portside_api::ApiError),
    #[error("could not create the catalog folder: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid search cursor")]
    Cursor,
}
