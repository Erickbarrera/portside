//! Files from the player's own copy of a game, for ports that run on them
//! (docs/specs/manifest-format.md, "game_files"). Copied after each install and update from
//! the copy used last time, else the first one a store installed; the game page shows where
//! they came from and lets the player choose a folder instead.

use std::path::{Path, PathBuf};

use portside_api::models::GameFiles;
use portside_assets::{Found, Store};
use portside_core::Os;
use portside_core::paths::{proton_prefix, resolve_placeholders, resolve_placeholders_in_prefix};
use portside_installer::{BuildFit, Installed};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::error::{CommandError, ErrorCode};
use crate::state::AppState;

/// A port's game files on this computer.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GameFilesStatus {
    /// The game as stores sell it.
    pub label: String,
    /// What the port needs, as the manifest lists it.
    pub files: Vec<String>,
    /// The installed version has them in place.
    pub ready: bool,
    /// Where they were last copied from.
    pub source: Option<Found>,
    /// Copies of the game that stores installed on this computer.
    pub found: Vec<Found>,
}

fn error(message: impl Into<String>) -> CommandError {
    CommandError {
        code: ErrorCode::GameFiles,
        message: message.into(),
    }
}

/// The port's `game_files`, if it needs any.
async fn spec(state: &AppState, port_id: &str) -> Result<Option<GameFiles>, CommandError> {
    let port = state.api.port(port_id).await?;
    Ok(port.manifest.and_then(|m| m.game_files))
}

/// Where the files go for this install: the manifest's `dest` for its OS (Windows inside
/// the prefix for a Proton install), else the install folder.
fn dest(state: &AppState, spec: &GameFiles, installed: &Installed) -> Option<PathBuf> {
    let prefix = (installed.fit == BuildFit::Proton)
        .then(|| proton_prefix(&state.paths, &installed.port_id))
        .flatten();
    let os = if prefix.is_some() {
        Os::Windows
    } else {
        state.platform.os
    };
    let d = spec.dest.as_ref();
    let template = match os {
        Os::Windows => d.and_then(|d| d.windows.clone()),
        Os::Macos => d.and_then(|d| d.macos.clone()),
        Os::Linux => d.and_then(|d| d.linux.clone()),
        _ => None,
    }
    .unwrap_or_else(|| "{install}/".to_owned());
    let install = Path::new(&installed.dir);
    match &prefix {
        Some(prefix) => resolve_placeholders_in_prefix(&template, prefix, Some(install)),
        None => resolve_placeholders(&template, os, Some(install)),
    }
}

fn recorded(state: &AppState, port_id: &str) -> Option<Found> {
    let (store, dir) = state
        .installs
        .installer()
        .library()
        .game_files_source(port_id)
        .ok()
        .flatten()?;
    Some(Found {
        store: Store::parse(&store)?,
        dir: PathBuf::from(dir),
    })
}

fn installed(state: &AppState, port_id: &str) -> Result<Installed, CommandError> {
    state
        .installs
        .installer()
        .library()
        .current(port_id)?
        .ok_or_else(|| portside_installer::InstallError::NotInstalled(port_id.to_owned()).into())
}

/// Copies the files from `from` (a folder holding them, or with them up to four levels
/// down) into place for the installed version, and remembers where they came from.
fn copy(
    state: &AppState,
    spec: &GameFiles,
    installed: &Installed,
    from: &Found,
) -> Result<(), CommandError> {
    let root = portside_assets::data_root(&from.dir, &spec.files).ok_or_else(|| {
        error(format!(
            "That folder doesn't have the files from {} that this game needs ({}).",
            spec.label,
            spec.files.join(", ")
        ))
    })?;
    let to = dest(state, spec, installed)
        .ok_or_else(|| error("This game doesn't say where its files go on this system."))?;
    portside_assets::copy_files(&root, &spec.files, &to)
        .map_err(|e| error(format!("Couldn't copy the files from {}: {e}", spec.label)))?;
    state.installs.installer().library().set_game_files_source(
        &installed.port_id,
        from.store.as_str(),
        &root.display().to_string(),
    )?;
    Ok(())
}

/// What the game page shows; None when the port needs no game files.
pub async fn status(
    app: &AppHandle,
    port_id: &str,
) -> Result<Option<GameFilesStatus>, CommandError> {
    let state = app.state::<AppState>();
    let Some(spec) = spec(&state, port_id).await? else {
        return Ok(None);
    };
    let app = app.clone();
    let port_id = port_id.to_owned();
    blocking(move || {
        let state = app.state::<AppState>();
        let ready = state
            .installs
            .installer()
            .library()
            .current(&port_id)?
            .and_then(|i| dest(&state, &spec, &i))
            .is_some_and(|d| portside_assets::has_files(&d, &spec.files));
        let found = portside_assets::find(&spec, &portside_core::paths::store_places());
        Ok(Some(GameFilesStatus {
            label: spec.label.clone(),
            files: spec.files.clone(),
            ready,
            source: recorded(&state, &port_id),
            found,
        }))
    })
    .await
}

/// Copies the game files from `dir` for the installed version (a copy the page listed, or a
/// folder the player chose).
pub async fn copy_from(
    app: &AppHandle,
    port_id: &str,
    store: Store,
    dir: PathBuf,
) -> Result<Option<GameFilesStatus>, CommandError> {
    let state = app.state::<AppState>();
    if state.installs.is_running(port_id) {
        return Err(CommandError {
            code: ErrorCode::InstallRunning,
            message: "Wait for the install to finish.".to_owned(),
        });
    }
    state.games.check_not_running(port_id)?;
    let Some(spec) = spec(&state, port_id).await? else {
        return Err(error("This game doesn't use files from another game."));
    };
    {
        let app = app.clone();
        let port_id = port_id.to_owned();
        blocking(move || {
            let state = app.state::<AppState>();
            let installed = installed(&state, &port_id)?;
            copy(&state, &spec, &installed, &Found { store, dir })
        })
        .await?;
    }
    status(app, port_id).await
}

/// After an install or update: copies the files from where they came from last time, else
/// from the first copy a store installed. Nothing found is fine; the game page says so.
pub async fn after_install(app: &AppHandle, installed: &Installed) {
    let state = app.state::<AppState>();
    let Ok(Some(spec)) = spec(&state, &installed.port_id).await else {
        return;
    };
    crate::installs::emit(
        app,
        &installed.port_id,
        crate::installs::InstallState::CopyingGameFiles,
    );
    let app = app.clone();
    let installed = installed.clone();
    let outcome = blocking(move || {
        let state = app.state::<AppState>();
        let from = recorded(&state, &installed.port_id)
            .filter(|f| portside_assets::has_files(&f.dir, &spec.files))
            .or_else(|| {
                portside_assets::find(&spec, &portside_core::paths::store_places())
                    .into_iter()
                    .next()
            });
        match from {
            Some(from) => copy(&state, &spec, &installed, &from),
            None => Ok(()),
        }
    })
    .await;
    if let Err(e) = outcome {
        // No paths: they'd say where the player's games are.
        eprintln!("couldn't copy game files: {:?}", e.code);
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| error(format!("Couldn't check the game files: {e}")))?
}
