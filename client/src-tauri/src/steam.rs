//! "Add to Steam" (client/docs/platforms.md): puts an installed game in Steam's library as a
//! shortcut that starts Portside with `--play <port>`. Started that way, Portside opens the
//! game page, starts the game and quits when it ends normally.

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use portside_steam::{Shortcut, Steam, SteamError};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::error::{CommandError, ErrorCode};
use crate::state::AppState;

/// How long Steam gets to close before adding gives up.
const CLOSE_LIMIT: Duration = Duration::from_secs(30);
/// Biggest picture downloaded for artwork.
const ART_LIMIT: usize = 15 * 1024 * 1024;

/// The game Steam asked for (`--play <port>`): the UI starts it once, and Portside quits
/// after it ends normally.
#[derive(Default)]
pub struct PlayRequest {
    /// Taken by the UI at start-up.
    pending: Mutex<Option<String>>,
    /// Kept: the game whose end quits Portside.
    quit_after: Option<String>,
}

impl PlayRequest {
    pub fn from_args(args: impl IntoIterator<Item = String>) -> PlayRequest {
        let port = play_arg(args);
        PlayRequest {
            pending: Mutex::new(port.clone()),
            quit_after: port,
        }
    }

    pub fn take(&self) -> Option<String> {
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    pub fn quits_after(&self, port_id: &str) -> bool {
        self.quit_after.as_deref() == Some(port_id)
    }
}

/// The port named by `--play <port>` or `--play=<port>`.
fn play_arg(args: impl IntoIterator<Item = String>) -> Option<String> {
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--play" {
            return args.next().filter(|p| !p.is_empty());
        }
        if let Some(p) = arg.strip_prefix("--play=") {
            return Some(p.to_owned()).filter(|p| !p.is_empty());
        }
    }
    None
}

/// The game page's view of Steam.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SteamStatus {
    /// Steam is installed and has been signed in to here.
    pub found: bool,
    /// This game is in Steam's library already.
    pub added: bool,
    /// Steam is open; adding means closing it for a moment.
    pub running: bool,
    /// Steam started Portside, so Portside can't close Steam (it would close too).
    pub started_by_steam: bool,
}

fn launch_options(port_id: &str) -> String {
    format!("--play {port_id}")
}

/// True when Steam started this process (Game Mode, Big Picture, or the library).
fn started_by_steam() -> bool {
    ["SteamGameId", "SteamAppId", "SteamGamepadUI"]
        .iter()
        .any(|v| std::env::var_os(v).is_some())
}

/// True on a TV-style screen run by Steam: Game Mode on a Steam Deck (Gamescope), or
/// started from Big Picture. Couch mode turns on by itself there (docs/ui.md).
pub fn big_screen() -> bool {
    std::env::var_os("SteamGamepadUI").is_some()
        || std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|d| d.eq_ignore_ascii_case("gamescope"))
}

/// Asks Steam to show its on-screen keyboard; it types into whatever has focus.
pub async fn show_keyboard(app: &AppHandle) -> bool {
    if !blocking(portside_steam::running).await {
        return false;
    }
    match app.opener().open_url("steam://open/keyboard", None::<&str>) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("could not open Steam's keyboard: {e}");
            false
        }
    }
}

/// The program a shortcut should start: the AppImage itself rather than its temporary
/// mount, else this executable.
fn portside_exe() -> Result<PathBuf, CommandError> {
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        return Ok(PathBuf::from(appimage));
    }
    std::env::current_exe().map_err(|e| CommandError {
        code: ErrorCode::Steam,
        message: format!("Couldn't find where Portside is installed: {e}"),
    })
}

impl From<SteamError> for CommandError {
    fn from(e: SteamError) -> Self {
        let code = match e {
            SteamError::NotFound => ErrorCode::SteamNotFound,
            _ => ErrorCode::Steam,
        };
        let mut message = e.to_string();
        if let Some(first) = message.get(..1) {
            message.replace_range(..1, &first.to_uppercase());
        }
        CommandError { code, message }
    }
}

pub async fn status(port_id: &str) -> SteamStatus {
    let port_id = port_id.to_owned();
    tokio::task::spawn_blocking(move || {
        let steam = Steam::find().ok();
        SteamStatus {
            found: steam.is_some(),
            added: steam
                .and_then(|s| s.find_shortcut(&launch_options(&port_id)).ok().flatten())
                .is_some(),
            running: portside_steam::running(),
            started_by_steam: started_by_steam(),
        }
    })
    .await
    .unwrap_or(SteamStatus {
        found: false,
        added: false,
        running: false,
        started_by_steam: started_by_steam(),
    })
}

/// Adds the installed game to Steam, with artwork from the catalog's picture when it has one.
/// With Steam open it fails with `E_STEAM_RUNNING` unless `restart_steam`, which closes Steam,
/// adds the game and opens Steam again.
pub async fn add(app: &AppHandle, port_id: &str, restart_steam: bool) -> Result<(), CommandError> {
    let state = app.state::<AppState>();
    let install = state
        .installs
        .installer()
        .library()
        .current(port_id)?
        .ok_or_else(|| {
            CommandError::from(portside_installer::InstallError::NotInstalled(
                port_id.to_owned(),
            ))
        })?;
    let steam = Steam::find()?;
    let exe = portside_exe()?;
    let shortcut = Shortcut {
        name: install.title.clone(),
        start_dir: exe.parent().map(PathBuf::from).unwrap_or_default(),
        exe,
        launch_options: launch_options(port_id),
    };
    // Fetched before Steam closes, so it's closed as briefly as possible.
    let picture = match state.catalog.get(port_id) {
        Ok(Some(summary)) => match summary.screenshot_url {
            Some(url) => download_picture(&url).await,
            None => None,
        },
        _ => None,
    };

    let was_running = blocking(portside_steam::running).await;
    if was_running {
        if !restart_steam || started_by_steam() {
            return Err(CommandError {
                code: ErrorCode::SteamRunning,
                message: "Steam is open. Close Steam to add games to it.".to_owned(),
            });
        }
        close_steam(app).await?;
    }

    let added = tokio::task::spawn_blocking(move || {
        let appid = steam.add(&shortcut)?;
        if let Some(picture) = picture
            && let Err(e) = portside_steam::write_art(&steam.grid_dir(), appid, &picture)
        {
            // The game is still added; Steam shows its own placeholder.
            eprintln!("could not make Steam artwork: {e}");
        }
        Ok::<_, SteamError>(())
    })
    .await;
    let added = added.map_err(|e| CommandError {
        code: ErrorCode::Steam,
        message: format!("Adding to Steam stopped: {e}"),
    });

    if was_running && let Err(e) = app.opener().open_url("steam://open/games", None::<&str>) {
        eprintln!("could not open Steam again: {e}");
    }
    Ok(added??)
}

async fn close_steam(app: &AppHandle) -> Result<(), CommandError> {
    app.opener()
        .open_url("steam://exit", None::<&str>)
        .map_err(|e| CommandError {
            code: ErrorCode::Steam,
            message: format!("Couldn't ask Steam to close: {e}"),
        })?;
    let deadline = tokio::time::Instant::now() + CLOSE_LIMIT;
    while tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if !blocking(portside_steam::running).await {
            // Give it a moment to finish writing its files.
            tokio::time::sleep(Duration::from_secs(1)).await;
            return Ok(());
        }
    }
    Err(CommandError {
        code: ErrorCode::SteamRunning,
        message: "Steam didn't close. Close it yourself, then try again.".to_owned(),
    })
}

/// Runs blocking work off the async runtime; the default value if it panicked.
async fn blocking<T: Default + Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(f).await.unwrap_or_default()
}

/// The catalog's picture for a game; None when it can't be had (artwork is optional).
async fn download_picture(url: &str) -> Option<Vec<u8>> {
    if !url.starts_with("https://") {
        return None;
    }
    let client = portside_core::http::client_builder()
        .user_agent(concat!("portside-desktop/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()
        .ok()?;
    let response = client.get(url).send().await.ok()?.error_for_status().ok()?;
    if response
        .content_length()
        .is_some_and(|n| n > ART_LIMIT as u64)
    {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    (bytes.len() <= ART_LIMIT).then(|| bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn reads_the_game_to_play() {
        assert_eq!(
            play_arg(args(&["--play", "example-rpg"])),
            Some("example-rpg".into())
        );
        assert_eq!(
            play_arg(args(&["--play=example-rpg"])),
            Some("example-rpg".into())
        );
        assert_eq!(play_arg(args(&["--play"])), None);
        assert_eq!(play_arg(args(&["--other"])), None);
    }

    #[test]
    fn the_request_is_taken_once_but_still_quits() {
        let req = PlayRequest::from_args(args(&["--play", "a"]));
        assert_eq!(req.take(), Some("a".into()));
        assert_eq!(req.take(), None);
        assert!(req.quits_after("a"));
        assert!(!req.quits_after("b"));
    }
}
