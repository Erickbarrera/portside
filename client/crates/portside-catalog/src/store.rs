use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use portside_api::models::{CatalogSnapshot, PortSummary};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::error::CatalogError;

mod embedded {
    refinery::embed_migrations!("migrations");
}

/// What the local copy holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheState {
    /// Catalog revision of the copy; `None` until the first sync.
    pub rev: Option<u64>,
    pub port_count: u64,
    /// ETag the catalog host sent with the copy, for conditional requests.
    pub etag: Option<String>,
    /// Unix seconds of the last successful check against the catalog host.
    pub last_synced_at: Option<u64>,
}

/// The local catalog. One connection behind a mutex: queries are small and fast.
pub struct Catalog {
    conn: Mutex<Connection>,
}

impl Catalog {
    /// Opens (or creates) the database file and runs pending migrations.
    pub fn open(path: &Path) -> Result<Catalog, CatalogError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    /// An empty in-memory catalog, for tests.
    pub fn open_in_memory() -> Result<Catalog, CatalogError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Catalog, CatalogError> {
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        embedded::migrations::runner().run(&mut conn)?;
        Ok(Catalog {
            conn: Mutex::new(conn),
        })
    }

    pub(crate) fn conn(&self) -> MutexGuard<'_, Connection> {
        // A panic while holding the lock leaves SQLite consistent (transactions roll back).
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn state(&self) -> Result<CacheState, CatalogError> {
        let conn = self.conn();
        let port_count: i64 = conn.query_row("SELECT count(*) FROM ports", [], |r| r.get(0))?;
        Ok(CacheState {
            rev: get_meta(&conn, "rev")?,
            port_count: u64::try_from(port_count).unwrap_or(0),
            etag: get_meta(&conn, "etag")?,
            last_synced_at: get_meta(&conn, "last_synced_at")?,
        })
    }

    /// One port's summary from the local copy.
    pub fn get(&self, id: &str) -> Result<Option<PortSummary>, CatalogError> {
        let conn = self.conn();
        let json: Option<String> = conn
            .query_row("SELECT summary FROM ports WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(json.map(|j| serde_json::from_str(&j)).transpose()?)
    }

    /// Replaces the whole copy with a snapshot, remembering the host's `etag` for it.
    pub fn replace_all(
        &self,
        snapshot: &CatalogSnapshot,
        etag: Option<&str>,
        now: u64,
    ) -> Result<(), CatalogError> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM ports", [])?;
        for port in &snapshot.ports {
            upsert(&tx, port)?;
        }
        set_meta(&tx, "rev", snapshot.rev)?;
        set_etag(&tx, etag)?;
        set_meta(&tx, "last_synced_at", now)?;
        tx.commit()?;
        Ok(())
    }

    /// Records a successful check that found nothing new.
    pub fn mark_checked(&self, now: u64) -> Result<(), CatalogError> {
        let conn = self.conn();
        set_meta(&conn, "last_synced_at", now)?;
        Ok(())
    }

    /// Like [`Catalog::mark_checked`], when the host sent the same catalog under a new ETag.
    pub fn mark_checked_with_etag(&self, etag: Option<&str>, now: u64) -> Result<(), CatalogError> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        set_etag(&tx, etag)?;
        set_meta(&tx, "last_synced_at", now)?;
        tx.commit()?;
        Ok(())
    }
}

fn upsert(tx: &Transaction<'_>, port: &PortSummary) -> Result<(), CatalogError> {
    let summary = serde_json::to_string(port)?;
    // ON CONFLICT (not REPLACE) so the FTS update trigger fires.
    tx.execute(
        "INSERT INTO ports (id, title, type, status, stars, summary)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (id) DO UPDATE SET
           title = excluded.title, type = excluded.type, status = excluded.status,
           stars = excluded.stars, summary = excluded.summary",
        params![
            port.id,
            port.title,
            port.port_type.as_str(),
            port.status.as_str(),
            port.stars,
            summary
        ],
    )?;
    tx.execute("DELETE FROM port_os WHERE port_id = ?1", [&port.id])?;
    for s in &port.os_status {
        tx.execute(
            "INSERT OR IGNORE INTO port_os (port_id, os) VALUES (?1, ?2)",
            params![port.id, s.os.as_str()],
        )?;
    }
    Ok(())
}

fn get_meta<T: std::str::FromStr>(conn: &Connection, key: &str) -> Result<Option<T>, CatalogError> {
    let value: Option<String> = conn
        .query_row("SELECT value FROM sync_state WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(value.and_then(|v| v.parse().ok()))
}

fn set_etag(conn: &Connection, etag: Option<&str>) -> Result<(), CatalogError> {
    match etag {
        Some(etag) => conn.execute(
            "INSERT INTO sync_state (key, value) VALUES ('etag', ?1)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [etag],
        )?,
        None => conn.execute("DELETE FROM sync_state WHERE key = 'etag'", [])?,
    };
    Ok(())
}

fn set_meta(conn: &Connection, key: &str, value: u64) -> Result<(), CatalogError> {
    conn.execute(
        "INSERT INTO sync_state (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value.to_string()],
    )?;
    Ok(())
}
