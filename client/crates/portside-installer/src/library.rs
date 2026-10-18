//! What is installed, in `{app_data}/library.db`.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use portside_api::models::ManifestBuild;
use portside_core::types::{Arch, Os};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::error::{IoContext, Result};
use crate::resolve::BuildFit;

mod embedded {
    refinery::embed_migrations!("migrations");
}

/// Where an install's checksum came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    /// The Portside API's record of the release.
    Portside,
    /// GitHub's own digest of the uploaded file.
    Github,
    /// A checksum file published with the release.
    ChecksumFile,
    /// The manifest says the release publishes no checksum.
    None,
}

/// One installed version of a port.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Installed {
    pub port_id: String,
    pub title: String,
    /// The release tag.
    pub version: String,
    /// Absolute path of the install folder.
    pub dir: String,
    pub os: Os,
    pub arch: Arch,
    pub fit: BuildFit,
    /// Release file it came from.
    pub asset: String,
    pub release_url: String,
    /// Download size in bytes.
    pub size: u64,
    pub sha256: Option<String>,
    pub verified: Verification,
    /// Unix seconds.
    pub installed_at: u64,
    /// The user agreed to run an app Apple has not checked.
    pub unsigned: bool,
    /// How to launch it, kept so the library works offline.
    pub build: ManifestBuild,
}

/// How long and how recently a port has been played, over every version.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PlayStats {
    pub port_id: String,
    /// Seconds played in total.
    pub total_secs: u64,
    /// Unix seconds the last session ended, if ever played.
    pub last_played: Option<u64>,
    pub sessions: u64,
}

pub struct Library {
    conn: Mutex<Connection>,
}

impl Library {
    pub fn open(path: &Path) -> Result<Library> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).at(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Library::setup(conn)
    }

    pub fn open_in_memory() -> Result<Library> {
        Library::setup(Connection::open_in_memory()?)
    }

    fn setup(mut conn: Connection) -> Result<Library> {
        conn.pragma_update(None, "foreign_keys", true)?;
        embedded::migrations::runner().run(&mut conn)?;
        Ok(Library {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Records an install and makes it the version that launches.
    pub fn add(&self, install: &Installed) -> Result<()> {
        let record = serde_json::to_string(install)?;
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO installs (port_id, version, title, installed_at, record) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (port_id, version) DO UPDATE SET
               title = excluded.title, installed_at = excluded.installed_at, record = excluded.record",
            params![install.port_id, install.version, install.title, install.installed_at as i64, record],
        )?;
        tx.execute(
            "INSERT INTO current_version (port_id, version) VALUES (?1, ?2)
             ON CONFLICT (port_id) DO UPDATE SET version = excluded.version",
            params![install.port_id, install.version],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The version of `port_id` that launches, if installed.
    pub fn current(&self, port_id: &str) -> Result<Option<Installed>> {
        let json: Option<String> = self
            .conn()
            .query_row(
                "SELECT i.record FROM current_version c
                 JOIN installs i ON i.port_id = c.port_id AND i.version = c.version
                 WHERE c.port_id = ?1",
                [port_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(json.map(|j| serde_json::from_str(&j)).transpose()?)
    }

    /// Every installed port's current version, by title.
    pub fn list(&self) -> Result<Vec<Installed>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT i.record FROM current_version c
             JOIN installs i ON i.port_id = c.port_id AND i.version = c.version
             ORDER BY i.title COLLATE NOCASE, i.port_id",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for json in rows {
            out.push(serde_json::from_str(&json?)?);
        }
        Ok(out)
    }

    /// Forgets every version of `port_id`; returns how many there were.
    pub fn remove(&self, port_id: &str) -> Result<usize> {
        let n = self
            .conn()
            .execute("DELETE FROM installs WHERE port_id = ?1", [port_id])?;
        Ok(n)
    }

    /// Remembers where `port_id`'s game files were copied from: a store name and a folder.
    pub fn set_game_files_source(&self, port_id: &str, store: &str, dir: &str) -> Result<()> {
        self.conn().execute(
            "INSERT INTO game_files (port_id, store, dir) VALUES (?1, ?2, ?3)
             ON CONFLICT (port_id) DO UPDATE SET store = excluded.store, dir = excluded.dir",
            params![port_id, store, dir],
        )?;
        Ok(())
    }

    /// Where `port_id`'s game files were last copied from, as `(store, folder)`.
    pub fn game_files_source(&self, port_id: &str) -> Result<Option<(String, String)>> {
        let row = self
            .conn()
            .query_row(
                "SELECT store, dir FROM game_files WHERE port_id = ?1",
                [port_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(row)
    }

    /// Records that `port_id` started running; returns the session to [`Library::touch_session`].
    pub fn start_session(&self, port_id: &str, version: &str, at: u64) -> Result<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO play_sessions (port_id, version, started_at, ended_at) VALUES (?1, ?2, ?3, ?3)",
            params![port_id, version, at as i64],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Moves a session's end to `at`: called while the game runs and once when it exits.
    pub fn touch_session(&self, id: i64, at: u64) -> Result<()> {
        self.conn().execute(
            "UPDATE play_sessions SET ended_at = max(ended_at, ?2) WHERE id = ?1",
            params![id, at as i64],
        )?;
        Ok(())
    }

    /// Play time for every port ever played.
    pub fn play_stats(&self) -> Result<Vec<PlayStats>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT port_id, sum(ended_at - started_at), max(ended_at), count(*)
             FROM play_sessions GROUP BY port_id ORDER BY port_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(PlayStats {
                port_id: r.get(0)?,
                total_secs: r.get::<_, i64>(1)?.max(0) as u64,
                last_played: Some(r.get::<_, i64>(2)?.max(0) as u64),
                sessions: r.get::<_, i64>(3)?.max(0) as u64,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Play time for one port; zero when never played.
    pub fn play_stats_for(&self, port_id: &str) -> Result<PlayStats> {
        let stats = self.conn().query_row(
            "SELECT coalesce(sum(ended_at - started_at), 0), max(ended_at), count(*)
             FROM play_sessions WHERE port_id = ?1",
            [port_id],
            |r| {
                Ok(PlayStats {
                    port_id: port_id.to_owned(),
                    total_secs: r.get::<_, i64>(0)?.max(0) as u64,
                    last_played: r.get::<_, Option<i64>>(1)?.map(|t| t.max(0) as u64),
                    sessions: r.get::<_, i64>(2)?.max(0) as u64,
                })
            },
        )?;
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_time_adds_up_per_port() -> Result<()> {
        let lib = Library::open_in_memory()?;
        assert_eq!(lib.play_stats_for("sm64")?.sessions, 0);
        assert_eq!(lib.play_stats_for("sm64")?.last_played, None);

        let a = lib.start_session("sm64", "v1", 1_000)?;
        lib.touch_session(a, 1_060)?;
        lib.touch_session(a, 1_120)?;
        // A late, out-of-order beat never moves the end back.
        lib.touch_session(a, 1_090)?;
        let b = lib.start_session("sm64", "v2", 5_000)?;
        lib.touch_session(b, 5_030)?;
        lib.start_session("soh", "9.0", 7_000)?;

        let sm64 = lib.play_stats_for("sm64")?;
        assert_eq!(sm64.total_secs, 150);
        assert_eq!(sm64.last_played, Some(5_030));
        assert_eq!(sm64.sessions, 2);

        let all = lib.play_stats()?;
        assert_eq!(lib.game_files_source("sm64")?, None);
        lib.set_game_files_source("sm64", "steam", "/a")?;
        lib.set_game_files_source("sm64", "folder", "/b")?;
        assert_eq!(
            lib.game_files_source("sm64")?,
            Some(("folder".into(), "/b".into()))
        );
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].port_id, "soh");
        assert_eq!(all[1].total_secs, 0);
        Ok(())
    }
}
