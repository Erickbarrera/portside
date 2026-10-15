//! Runs installed games, records play time and reports them to the UI as `game-changed`
//! events.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use portside_installer::PlayStats;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::Notify;

use crate::error::{CommandError, ErrorCode};
use crate::state::AppState;

/// How often a running game's play time is saved, so a crash loses at most this much.
const HEARTBEAT: Duration = Duration::from_secs(60);

/// Where a game has got to.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum GameState {
    /// Unix seconds it started.
    Running { started_at: u64 },
    /// Asked to quit; it is killed if it hasn't after a few seconds.
    Stopping { started_at: u64 },
    /// It ended. `code` is None when it was ended by a signal.
    Exited {
        code: Option<i32>,
        stopped: bool,
        stats: PlayStats,
    },
}

/// Sent when a game starts, is asked to stop, or exits.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct GameChanged {
    pub port_id: String,
    pub state: GameState,
}

struct Game {
    stop: Arc<Notify>,
    started_at: u64,
    stopping: bool,
}

pub struct Games {
    logs_dir: PathBuf,
    running: Mutex<HashMap<String, Game>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

fn already_running() -> CommandError {
    CommandError {
        code: ErrorCode::GameRunning,
        message: "This game is running. Quit it first.".to_owned(),
    }
}

impl Games {
    pub fn new(logs_dir: PathBuf) -> Games {
        Games {
            logs_dir,
            running: Mutex::new(HashMap::new()),
        }
    }

    /// Games running now.
    pub fn running(&self) -> Vec<GameChanged> {
        lock(&self.running)
            .iter()
            .map(|(port_id, g)| GameChanged {
                port_id: port_id.clone(),
                state: if g.stopping {
                    GameState::Stopping {
                        started_at: g.started_at,
                    }
                } else {
                    GameState::Running {
                        started_at: g.started_at,
                    }
                },
            })
            .collect()
    }

    pub fn is_running(&self, port_id: &str) -> bool {
        lock(&self.running).contains_key(port_id)
    }

    /// Fails when the game is running, for actions that would change its files.
    pub fn check_not_running(&self, port_id: &str) -> Result<(), CommandError> {
        if self.is_running(port_id) {
            return Err(already_running());
        }
        Ok(())
    }

    /// Where a game's output goes: `{app_data}/logs/{port_id}.log`, replaced on each run.
    pub fn log_for(&self, port_id: &str) -> PathBuf {
        let name: String = port_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.logs_dir.join(format!("{name}.log"))
    }
}

fn emit(app: &AppHandle, port_id: &str, state: GameState) {
    let event = GameChanged {
        port_id: port_id.to_owned(),
        state,
    };
    if let Err(e) = event.emit(app) {
        eprintln!("could not tell the UI about {port_id}: {e}");
    }
}

/// Starts the installed version of `port_id` and follows it in the background.
pub fn launch(app: &AppHandle, port_id: String) -> Result<(), CommandError> {
    let state = app.state::<AppState>();
    if state.installs.is_running(&port_id) {
        return Err(CommandError {
            code: ErrorCode::InstallRunning,
            message: "Wait for the install to finish.".to_owned(),
        });
    }
    let stop = Arc::new(Notify::new());
    let started_at = now();
    {
        let mut running = lock(&state.games.running);
        if running.contains_key(&port_id) {
            return Err(already_running());
        }
        running.insert(
            port_id.clone(),
            Game {
                stop: stop.clone(),
                started_at,
                stopping: false,
            },
        );
    }
    let started = (|| {
        let library = state.installs.installer().library();
        let install = library.current(&port_id)?.ok_or_else(|| {
            CommandError::from(portside_installer::InstallError::NotInstalled(
                port_id.clone(),
            ))
        })?;
        let command = portside_launcher::command_for(&install)?;
        let game = portside_launcher::spawn(&command, Some(&state.games.log_for(&port_id)))?;
        Ok::<_, CommandError>((install, game))
    })();
    let (install, game) = match started {
        Ok(s) => s,
        Err(e) => {
            lock(&state.games.running).remove(&port_id);
            return Err(e);
        }
    };
    let library = state.installs.installer().library();
    let session = match library.start_session(&port_id, &install.version, started_at) {
        Ok(id) => Some(id),
        Err(e) => {
            // The game still runs; this session just isn't counted.
            eprintln!("could not record play time for {port_id}: {e}");
            None
        }
    };
    emit(app, &port_id, GameState::Running { started_at });

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let library = state.installs.installer().library();
        let touch = |at| {
            if let Some(id) = session
                && let Err(e) = library.touch_session(id, at)
            {
                eprintln!("could not record play time for {port_id}: {e}");
            }
        };
        let wait = game.wait(&stop);
        tokio::pin!(wait);
        let mut beat = tokio::time::interval(HEARTBEAT);
        beat.tick().await;
        let exit = loop {
            tokio::select! {
                exit = &mut wait => break exit,
                _ = beat.tick() => touch(now()),
            }
        };
        touch(now());
        lock(&state.games.running).remove(&port_id);
        let (code, stopped) = match exit {
            Ok(exit) => (exit.code, exit.stopped),
            Err(e) => {
                eprintln!("lost track of {port_id}: {e}");
                (None, false)
            }
        };
        let stats = library.play_stats_for(&port_id).unwrap_or_else(|e| {
            eprintln!("could not read play time for {port_id}: {e}");
            PlayStats {
                port_id: port_id.clone(),
                ..PlayStats::default()
            }
        });
        emit(
            &app,
            &port_id,
            GameState::Exited {
                code,
                stopped,
                stats,
            },
        );
        crate::saves::spawn_after_exit(&app, &port_id);
    });
    Ok(())
}

/// Asks a running game to quit; returns false when it isn't running.
pub fn stop(app: &AppHandle, port_id: &str) -> bool {
    let state = app.state::<AppState>();
    let started_at = {
        let mut running = lock(&state.games.running);
        let Some(game) = running.get_mut(port_id) else {
            return false;
        };
        game.stopping = true;
        game.stop.notify_one();
        game.started_at
    };
    emit(app, port_id, GameState::Stopping { started_at });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_names_stay_in_the_logs_folder() {
        let games = Games::new(PathBuf::from("/logs"));
        assert_eq!(
            games.log_for("sm64coopdx"),
            PathBuf::from("/logs/sm64coopdx.log")
        );
        assert_eq!(games.log_for("../x"), PathBuf::from("/logs/___x.log"));
    }
}
