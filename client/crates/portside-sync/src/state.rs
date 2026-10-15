//! This device's side of cloud saves, in `{app_data}/saves.db`.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::error::{IoContext, Result};
use crate::vault::FileEntry;

mod embedded {
    refinery::embed_migrations!("migrations");
}

/// A game's `saves` section for this OS, before placeholders are resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveTemplate {
    pub root: String,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// What this device knows about one game.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameRow {
    pub base_id: Option<String>,
    pub base_files: BTreeMap<String, FileEntry>,
    pub custom_root: Option<String>,
    pub template: Option<SaveTemplate>,
    /// Unix seconds.
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
}

pub struct LocalState {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for LocalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LocalState")
    }
}

impl LocalState {
    pub fn open(path: &Path) -> Result<LocalState> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).at(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        LocalState::setup(conn)
    }

    pub fn open_in_memory() -> Result<LocalState> {
        LocalState::setup(Connection::open_in_memory()?)
    }

    fn setup(mut conn: Connection) -> Result<LocalState> {
        embedded::migrations::runner().run(&mut conn)?;
        Ok(LocalState {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn()
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn set_meta(&self, key: &str, value: Option<&str>) -> Result<()> {
        let conn = self.conn();
        match value {
            Some(v) => conn.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![key, v],
            )?,
            None => conn.execute("DELETE FROM meta WHERE key = ?1", [key])?,
        };
        Ok(())
    }

    /// This device's id, made on first use.
    pub fn device_id(&self) -> Result<String> {
        if let Some(id) = self.meta("device_id")? {
            return Ok(id);
        }
        let id = ulid::Ulid::new().to_string();
        self.set_meta("device_id", Some(&id))?;
        Ok(id)
    }

    /// Forgets which snapshot each game matched: blob ids differ between vaults, so this
    /// runs whenever the device switches vaults.
    pub fn forget_bases(&self) -> Result<()> {
        self.conn().execute(
            "UPDATE games SET base_id = NULL, base_files = '{}', last_sync = NULL, last_error = NULL",
            [],
        )?;
        Ok(())
    }

    pub fn game(&self, port_id: &str) -> Result<GameRow> {
        let row = self
            .conn()
            .query_row(
                "SELECT base_id, base_files, custom_root, rules, last_sync, last_error
                 FROM games WHERE port_id = ?1",
                [port_id],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get::<_, Option<i64>>(4)?,
                        r.get::<_, Option<String>>(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((base_id, files, custom_root, rules, last_sync, last_error)) = row else {
            return Ok(GameRow::default());
        };
        Ok(GameRow {
            base_id,
            base_files: serde_json::from_str(&files)?,
            custom_root,
            template: rules.map(|r| serde_json::from_str(&r)).transpose()?,
            last_sync,
            last_error,
        })
    }

    fn ensure(conn: &Connection, port_id: &str) -> Result<()> {
        conn.execute(
            "INSERT INTO games (port_id) VALUES (?1) ON CONFLICT (port_id) DO NOTHING",
            [port_id],
        )?;
        Ok(())
    }

    pub fn set_base(
        &self,
        port_id: &str,
        base_id: &str,
        files: &BTreeMap<String, FileEntry>,
    ) -> Result<()> {
        let conn = self.conn();
        LocalState::ensure(&conn, port_id)?;
        conn.execute(
            "UPDATE games SET base_id = ?2, base_files = ?3 WHERE port_id = ?1",
            params![port_id, base_id, serde_json::to_string(files)?],
        )?;
        Ok(())
    }

    pub fn set_custom_root(&self, port_id: &str, root: Option<&str>) -> Result<()> {
        let conn = self.conn();
        LocalState::ensure(&conn, port_id)?;
        conn.execute(
            "UPDATE games SET custom_root = ?2 WHERE port_id = ?1",
            params![port_id, root],
        )?;
        Ok(())
    }

    pub fn set_template(&self, port_id: &str, template: Option<&SaveTemplate>) -> Result<()> {
        let conn = self.conn();
        LocalState::ensure(&conn, port_id)?;
        let json = template.map(serde_json::to_string).transpose()?;
        conn.execute(
            "UPDATE games SET rules = ?2 WHERE port_id = ?1",
            params![port_id, json],
        )?;
        Ok(())
    }

    pub fn set_result(&self, port_id: &str, at: i64, error: Option<&str>) -> Result<()> {
        let conn = self.conn();
        LocalState::ensure(&conn, port_id)?;
        if error.is_some() {
            conn.execute(
                "UPDATE games SET last_error = ?2 WHERE port_id = ?1",
                params![port_id, error],
            )?;
        } else {
            conn.execute(
                "UPDATE games SET last_sync = ?2, last_error = NULL WHERE port_id = ?1",
                params![port_id, at],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn games_remember_their_base_and_folder() {
        let state = LocalState::open_in_memory().expect("db");
        let id = state.device_id().expect("id");
        assert_eq!(state.device_id().expect("id"), id);
        assert_eq!(state.game("p").expect("game"), GameRow::default());
        let mut files = BTreeMap::new();
        files.insert(
            "a.sav".to_owned(),
            FileEntry {
                size: 1,
                mtime: 2,
                blob: "b".to_owned(),
            },
        );
        state.set_base("p", "s1", &files).expect("base");
        state.set_custom_root("p", Some("/x")).expect("root");
        state.set_result("p", 10, None).expect("ok");
        state.set_result("p", 11, Some("boom")).expect("err");
        let game = state.game("p").expect("game");
        assert_eq!(game.base_id.as_deref(), Some("s1"));
        assert_eq!(game.base_files, files);
        assert_eq!(game.custom_root.as_deref(), Some("/x"));
        assert_eq!(game.last_sync, Some(10));
        assert_eq!(game.last_error.as_deref(), Some("boom"));
        state.forget_bases().expect("forget");
        let game = state.game("p").expect("game");
        assert_eq!(game.base_id, None);
        assert_eq!(game.custom_root.as_deref(), Some("/x"));
    }
}
