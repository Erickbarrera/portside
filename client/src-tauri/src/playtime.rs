//! Play time on every device (docs/specs/save-sync-protocol.md, "Play time"): this device's
//! sessions from `library.db`, plus what other devices left in the cloud saves folder.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, MutexGuard, PoisonError};

use portside_installer::PlayStats;
use portside_sync::{PlaytimeEntry, PlaytimeRecord};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::state::AppState;

/// Sent when play time from other devices changes what the UI shows.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct PlaytimeChanged {
    /// Every game ever played, on any device.
    pub stats: Vec<PlayStats>,
}

/// Other devices' play time, added up per game, as last read from the vault.
#[derive(Default)]
pub struct Playtime {
    others: Mutex<HashMap<String, PlaytimeEntry>>,
}

impl Playtime {
    fn others(&self) -> MutexGuard<'_, HashMap<String, PlaytimeEntry>> {
        self.others.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// One game's play time here plus on other devices.
    pub fn with_others(&self, local: PlayStats) -> PlayStats {
        let other = self.others().get(&local.port_id).copied();
        add(local, other)
    }

    /// Every game's play time here plus on other devices, by port id.
    pub fn all(&self, local: Vec<PlayStats>) -> Vec<PlayStats> {
        let others = self.others();
        let mut out: BTreeMap<String, PlayStats> = local
            .into_iter()
            .map(|s| {
                (
                    s.port_id.clone(),
                    add(s.clone(), others.get(&s.port_id).copied()),
                )
            })
            .collect();
        for (port_id, entry) in others.iter() {
            out.entry(port_id.clone()).or_insert_with(|| {
                add(
                    PlayStats {
                        port_id: port_id.clone(),
                        ..PlayStats::default()
                    },
                    Some(*entry),
                )
            });
        }
        out.into_values().collect()
    }

    fn set(&self, others: HashMap<String, PlaytimeEntry>) {
        *self.others() = others;
    }
}

fn add(mut local: PlayStats, other: Option<PlaytimeEntry>) -> PlayStats {
    if let Some(o) = other {
        local.total_secs += o.total_secs;
        local.sessions += o.sessions;
        local.last_played = local.last_played.max(Some(o.last_played));
    }
    local
}

/// What this device writes to `playtime/<device id>.json`.
fn own_games(local: &[PlayStats]) -> BTreeMap<String, PlaytimeEntry> {
    local
        .iter()
        .filter_map(|s| {
            Some((
                s.port_id.clone(),
                PlaytimeEntry {
                    total_secs: s.total_secs,
                    last_played: s.last_played?,
                    sessions: s.sessions,
                },
            ))
        })
        .collect()
}

/// Other devices' play time added up per game.
fn others_from(records: &[PlaytimeRecord], this_device: &str) -> HashMap<String, PlaytimeEntry> {
    let mut out: HashMap<String, PlaytimeEntry> = HashMap::new();
    for record in records.iter().filter(|r| r.device_id != this_device) {
        for (port_id, e) in &record.games {
            out.entry(port_id.clone())
                .and_modify(|sum| {
                    sum.total_secs += e.total_secs;
                    sum.sessions += e.sessions;
                    sum.last_played = sum.last_played.max(e.last_played);
                })
                .or_insert(*e);
        }
    }
    out
}

fn emit(app: &AppHandle, stats: Vec<PlayStats>) {
    if let Err(e) = (PlaytimeChanged { stats }).emit(app) {
        eprintln!("could not tell the UI about play time: {e}");
    }
}

/// Writes this device's play time to the cloud saves folder and reads the other devices'.
/// Does nothing when cloud saves are off.
pub async fn share(app: &AppHandle) {
    let handle = app.clone();
    let shared = tokio::task::spawn_blocking(move || {
        let state = handle.state::<AppState>();
        if !state.saves.enabled() {
            return Ok(None);
        }
        let local = state.installs.installer().library().play_stats()?;
        let records = state.saves.share_playtime(own_games(&local))?;
        state
            .playtime
            .set(others_from(&records, state.saves.device_id()));
        anyhow::Ok(Some(state.playtime.all(local)))
    })
    .await;
    match shared {
        Ok(Ok(Some(stats))) => emit(app, stats),
        Ok(Ok(None)) => {}
        Ok(Err(e)) => eprintln!("could not sync play time: {e}"),
        Err(e) => eprintln!("syncing play time stopped unexpectedly: {e}"),
    }
}

pub fn spawn_share(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move { share(&app).await });
}

/// Cloud saves were turned off: show only this device's play time again.
pub fn forget_others(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.playtime.set(HashMap::new());
    match state.installs.installer().library().play_stats() {
        Ok(local) => emit(app, local),
        Err(e) => eprintln!("could not read play time: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(device: &str, port_id: &str, total_secs: u64, last_played: u64) -> PlaytimeRecord {
        PlaytimeRecord {
            device_id: device.to_owned(),
            device_name: device.to_owned(),
            updated_at: "2026-10-17T22:00:00Z".to_owned(),
            games: BTreeMap::from([(
                port_id.to_owned(),
                PlaytimeEntry {
                    total_secs,
                    last_played,
                    sessions: 1,
                },
            )]),
        }
    }

    fn stats(port_id: &str, total_secs: u64, last_played: Option<u64>) -> PlayStats {
        PlayStats {
            port_id: port_id.to_owned(),
            total_secs,
            last_played,
            sessions: u64::from(last_played.is_some()),
        }
    }

    #[test]
    fn play_time_adds_up_over_devices() {
        let playtime = Playtime::default();
        let records = [
            record("MAC", "a", 600, 5_000),
            record("DECK", "a", 300, 9_000),
            record("DECK", "b", 60, 2_000),
            record("LAPTOP", "a", 100, 1_000),
        ];
        // This device's own file is already in its local sessions.
        playtime.set(others_from(&records, "MAC"));

        let a = playtime.with_others(stats("a", 600, Some(5_000)));
        assert_eq!(a.total_secs, 1_000);
        assert_eq!(a.sessions, 3);
        assert_eq!(a.last_played, Some(9_000));

        let all = playtime.all(vec![stats("a", 600, Some(5_000)), stats("c", 0, None)]);
        let ids: Vec<_> = all.iter().map(|s| s.port_id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);
        assert_eq!(all[1].total_secs, 60);
        assert_eq!(all[1].last_played, Some(2_000));
        assert_eq!(all[2].last_played, None);
    }

    #[test]
    fn only_played_games_are_shared() {
        let games = own_games(&[stats("a", 30, Some(100)), stats("b", 0, None)]);
        assert_eq!(games.keys().collect::<Vec<_>>(), ["a"]);
    }
}
