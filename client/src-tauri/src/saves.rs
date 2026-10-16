//! Cloud saves (docs/specs/save-sync-protocol.md): the vault this device uses, syncing a
//! game before it starts and after it exits, and the `saves-changed` event.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use anyhow::Context;
use portside_core::paths::{
    AppPaths, proton_prefix, resolve_placeholders, resolve_placeholders_in_prefix, synced_folders,
};
use portside_core::{Os, Platform};
use portside_installer::BuildFit;
use portside_sync::{
    Device, FolderStorage, LocalState, SaveRules, SaveTemplate, SnapshotInfo, SyncError,
    SyncOutcome, SyncState, Syncer, Vault,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::error::{CommandError, ErrorCode};
use crate::state::AppState;

/// Longest a sync may hold up a game's start.
const BEFORE_LAUNCH_LIMIT: Duration = Duration::from_secs(60);
/// Restored-over saves are kept this long.
const TRASH_DAYS: u64 = 30;

/// Where a game's saves stand on this device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SaveStatus {
    /// Cloud saves aren't set up on this device.
    Off,
    /// Portside doesn't know where this game keeps its saves on this system.
    NoFolder,
    /// Set up, but this game hasn't synced on this device yet.
    NotSynced,
    Syncing,
    Synced,
    /// Two devices have different saves; the player has to keep one.
    Conflict,
    /// The cloud folder can't be reached right now.
    Unavailable,
    Error,
}

/// A game's cloud saves, for the game page.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GameSaves {
    pub port_id: String,
    pub status: SaveStatus,
    /// The save folder on this device.
    pub folder: Option<String>,
    /// The player chose the folder (instead of the game's listing).
    pub custom_folder: bool,
    /// Unix seconds of the last finished sync.
    pub last_sync: Option<i64>,
    /// What the last sync did.
    pub last_result: Option<SyncState>,
    pub message: Option<String>,
    pub current: Option<SnapshotInfo>,
    /// The sides of a conflict, newest first.
    pub conflict: Vec<SnapshotInfo>,
}

/// Sent whenever a game's cloud saves change state (syncing, synced, conflict…).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct SavesChanged {
    pub saves: GameSaves,
}

/// A folder offered for the vault.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FolderSuggestion {
    pub label: String,
    pub path: String,
    /// It already holds Portside saves (from another device).
    pub has_saves: bool,
}

/// Cloud saves on this device.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SavesStatus {
    pub enabled: bool,
    /// The folder the vault is in.
    pub location: Option<String>,
    /// Set when the vault can't be opened right now.
    pub problem: Option<String>,
    pub device_name: String,
    /// Synced folders found on this computer.
    pub suggestions: Vec<FolderSuggestion>,
}

pub struct Saves {
    local: Arc<LocalState>,
    trash_root: PathBuf,
    device: Device,
    vault: Mutex<Option<Arc<Vault>>>,
    /// One sync per game at a time.
    locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    last: Mutex<HashMap<String, GameSaves>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn device_name() -> String {
    let host = gethostname::gethostname().to_string_lossy().into_owned();
    let host = host.trim_end_matches(".local").replace('-', " ");
    if host.trim().is_empty() {
        format!("{} computer", os_label(Platform::current().os))
    } else {
        host
    }
}

fn os_label(os: Os) -> &'static str {
    match os {
        Os::Windows => "Windows",
        Os::Macos => "Mac",
        Os::Linux => "Linux",
        Os::Android => "Android",
    }
}

fn blocking_error(e: tokio::task::JoinError) -> CommandError {
    CommandError {
        code: ErrorCode::Saves,
        message: format!("Cloud saves stopped unexpectedly: {e}"),
    }
}

impl Saves {
    pub fn open(paths: &AppPaths) -> anyhow::Result<Saves> {
        let local = LocalState::open(&paths.data_dir.join("saves.db"))
            .context("opening the cloud saves database")?;
        let device = Device {
            id: local.device_id().context("reading this device's id")?,
            name: device_name(),
            os: Platform::current().os,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
        };
        Ok(Saves {
            local: Arc::new(local),
            trash_root: paths.data_dir.join("saves-trash"),
            device,
            vault: Mutex::new(None),
            locks: Mutex::new(HashMap::new()),
            last: Mutex::new(HashMap::new()),
        })
    }

    fn location(&self) -> Option<String> {
        self.local.meta("location").ok().flatten()
    }

    pub fn enabled(&self) -> bool {
        self.location().is_some()
    }

    /// The open vault. Blocking.
    fn vault(&self) -> Result<Arc<Vault>, SyncError> {
        if let Some(v) = lock(&self.vault).clone() {
            return Ok(v);
        }
        let location = self.location().ok_or(SyncError::NotEnabled)?;
        let vault_id = self.local.meta("vault_id")?.ok_or(SyncError::NotEnabled)?;
        let vault = Arc::new(Vault::open(Box::new(FolderStorage::new(location)))?);
        if vault.id() != vault_id {
            return Err(SyncError::Corrupt(
                "the saves in the folder were replaced by other saves".to_owned(),
            ));
        }
        *lock(&self.vault) = Some(vault.clone());
        Ok(vault)
    }

    pub fn status(&self) -> SavesStatus {
        let location = self.location();
        let problem = if location.is_some() {
            self.vault().err().map(|e| e.to_string())
        } else {
            None
        };
        let suggestions = synced_folders()
            .into_iter()
            .map(|f| FolderSuggestion {
                has_saves: Vault::inspect(&FolderStorage::new(&f.path))
                    .ok()
                    .flatten()
                    .is_some(),
                label: f.label,
                path: f.path.display().to_string(),
            })
            .collect();
        SavesStatus {
            enabled: location.is_some(),
            location,
            problem,
            device_name: self.device.name.clone(),
            suggestions,
        }
    }

    /// Turns cloud saves on with the vault in `location`, making one if it has none.
    /// Blocking.
    pub fn use_folder(&self, location: &Path) -> Result<(), SyncError> {
        let vault = Vault::open_or_create(Box::new(FolderStorage::new(location)))?;
        if self.local.meta("vault_id")?.as_deref() != Some(vault.id()) {
            self.local.forget_bases()?;
        }
        self.local
            .set_meta("location", Some(&location.display().to_string()))?;
        self.local.set_meta("vault_id", Some(vault.id()))?;
        *lock(&self.vault) = Some(Arc::new(vault));
        lock(&self.last).clear();
        Ok(())
    }

    /// Stops syncing on this device. The vault and the saves on disk are left alone.
    pub fn turn_off(&self) -> Result<(), SyncError> {
        self.local.set_meta("location", None)?;
        *lock(&self.vault) = None;
        lock(&self.last).clear();
        Ok(())
    }

    pub fn devices(&self) -> Result<Vec<portside_sync::DeviceRecord>, SyncError> {
        self.vault()?.devices()
    }

    pub fn set_custom_folder(&self, port_id: &str, folder: Option<&str>) -> Result<(), SyncError> {
        lock(&self.last).remove(port_id);
        self.local.set_custom_root(port_id, folder)
    }

    fn game_lock(&self, port_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        lock(&self.locks)
            .entry(port_id.to_owned())
            .or_default()
            .clone()
    }
}

/// Where a game's saves are on this device, and the installed version.
async fn rules_for(
    app: &AppHandle,
    port_id: &str,
) -> Result<(Option<SaveRules>, Option<String>), CommandError> {
    let state = app.state::<AppState>();
    let saves = &state.saves;
    let row = saves.local.game(port_id)?;
    let installed = state.installs.installer().library().current(port_id)?;
    let version = installed.as_ref().map(|i| i.version.clone());
    if let Some(root) = row.custom_root {
        return Ok((
            Some(SaveRules {
                root: PathBuf::from(root),
                include: Vec::new(),
                exclude: Vec::new(),
            }),
            version,
        ));
    }
    // A Windows build run through Proton keeps Windows saves, inside its prefix.
    let prefix = installed
        .as_ref()
        .filter(|i| i.fit == BuildFit::Proton)
        .and_then(|i| proton_prefix(&state.paths, &i.port_id));
    let os = if prefix.is_some() {
        Os::Windows
    } else {
        state.platform.os
    };
    // The listing may have gained a save folder since last time; offline, use the copy.
    let template = match state.api.port(port_id).await {
        Ok(port) => {
            let template = port.manifest.and_then(|m| m.saves).and_then(|s| {
                let roots = s.root;
                let root = match os {
                    Os::Windows => roots.windows,
                    Os::Macos => roots.macos,
                    Os::Linux => roots.linux,
                    Os::Android => roots.android,
                }?;
                Some(SaveTemplate {
                    root,
                    include: s.include.unwrap_or_default(),
                    exclude: s.exclude.unwrap_or_default(),
                })
            });
            saves.local.set_template(port_id, template.as_ref())?;
            template
        }
        Err(_) => row.template,
    };
    let rules = template.and_then(|t| {
        let install = installed.as_ref().map(|i| PathBuf::from(&i.dir));
        Some(SaveRules {
            root: match &prefix {
                Some(prefix) => {
                    resolve_placeholders_in_prefix(&t.root, prefix, install.as_deref())?
                }
                None => resolve_placeholders(&t.root, os, install.as_deref())?,
            },
            include: t.include,
            exclude: t.exclude,
        })
    });
    Ok((rules, version))
}

fn view(
    port_id: &str,
    rules: Option<&SaveRules>,
    custom: bool,
    status: SaveStatus,
    row: &portside_sync::GameRow,
) -> GameSaves {
    GameSaves {
        port_id: port_id.to_owned(),
        status,
        folder: rules.map(|r| r.root.display().to_string()),
        custom_folder: custom,
        last_sync: row.last_sync,
        last_result: None,
        message: None,
        current: None,
        conflict: Vec::new(),
    }
}

fn emit(app: &AppHandle, saves: &GameSaves) {
    let event = SavesChanged {
        saves: saves.clone(),
    };
    if let Err(e) = event.emit(app) {
        eprintln!(
            "could not tell the UI about saves for {}: {e}",
            saves.port_id
        );
    }
}

/// A game's cloud saves as last known, without syncing.
pub async fn game(app: &AppHandle, port_id: &str) -> Result<GameSaves, CommandError> {
    let state = app.state::<AppState>();
    let saves = &state.saves;
    let row = saves.local.game(port_id)?;
    let custom = row.custom_root.is_some();
    if !saves.enabled() {
        return Ok(view(port_id, None, custom, SaveStatus::Off, &row));
    }
    let (rules, _) = rules_for(app, port_id).await?;
    let Some(rules) = rules else {
        return Ok(view(port_id, None, custom, SaveStatus::NoFolder, &row));
    };
    if let Some(last) = lock(&saves.last).get(port_id) {
        return Ok(last.clone());
    }
    let status = match (&row.last_error, row.last_sync) {
        (Some(_), _) => SaveStatus::Error,
        (None, Some(_)) => SaveStatus::Synced,
        (None, None) => SaveStatus::NotSynced,
    };
    let mut out = view(port_id, Some(&rules), custom, status, &row);
    out.message = row.last_error;
    Ok(out)
}

enum Action {
    Sync,
    Keep(String),
}

async fn run(app: &AppHandle, port_id: &str, action: Action) -> Result<GameSaves, CommandError> {
    let state = app.state::<AppState>();
    let saves = &state.saves;
    let row = saves.local.game(port_id)?;
    let custom = row.custom_root.is_some();
    if !saves.enabled() {
        return Err(SyncError::NotEnabled.into());
    }
    // A running game may be writing its saves; it syncs when it exits.
    state.games.check_not_running(port_id)?;
    let (rules, version) = rules_for(app, port_id).await?;
    let Some(rules) = rules else {
        return Ok(view(port_id, None, custom, SaveStatus::NoFolder, &row));
    };
    let game_lock = saves.game_lock(port_id);
    let _held = game_lock.lock().await;
    emit(
        app,
        &view(port_id, Some(&rules), custom, SaveStatus::Syncing, &row),
    );

    let handle = app.clone();
    let id = port_id.to_owned();
    let job_rules = rules.clone();
    let result = tokio::task::spawn_blocking(move || {
        let state = handle.state::<AppState>();
        let saves = &state.saves;
        let vault = saves.vault()?;
        let syncer = Syncer {
            vault: &vault,
            state: &saves.local,
            device: &saves.device,
            trash_root: &saves.trash_root,
        };
        portside_sync::prune_trash(&saves.trash_root, Duration::from_secs(TRASH_DAYS * 86_400));
        match action {
            Action::Sync => syncer.sync(&id, &job_rules, version.as_deref()),
            Action::Keep(snapshot) => syncer.keep(&id, &job_rules, &snapshot, version.as_deref()),
        }
    })
    .await
    .map_err(blocking_error)?;

    let row = saves.local.game(port_id)?;
    let out = match result {
        Ok(SyncOutcome {
            state: sync_state,
            current,
            conflict,
        }) => GameSaves {
            status: if sync_state == SyncState::Conflict {
                SaveStatus::Conflict
            } else {
                SaveStatus::Synced
            },
            last_result: Some(sync_state),
            current,
            conflict,
            ..view(port_id, Some(&rules), custom, SaveStatus::Synced, &row)
        },
        Err(e) => {
            if matches!(e, SyncError::Unavailable(_) | SyncError::Corrupt(_)) {
                // Try opening the vault afresh next time (folder back, key changed).
                *lock(&saves.vault) = None;
            }
            let status = match e {
                SyncError::Unavailable(_) | SyncError::NotDownloaded(_) => SaveStatus::Unavailable,
                SyncError::NotEnabled => SaveStatus::Off,
                _ => SaveStatus::Error,
            };
            GameSaves {
                message: Some(e.to_string()),
                ..view(port_id, Some(&rules), custom, status, &row)
            }
        }
    };
    lock(&saves.last).insert(port_id.to_owned(), out.clone());
    emit(app, &out);
    Ok(out)
}

pub async fn sync(app: &AppHandle, port_id: &str) -> Result<GameSaves, CommandError> {
    run(app, port_id, Action::Sync).await
}

pub async fn keep(
    app: &AppHandle,
    port_id: &str,
    snapshot_id: String,
) -> Result<GameSaves, CommandError> {
    run(app, port_id, Action::Keep(snapshot_id)).await
}

/// Syncs before a game starts. A conflict stops the launch so the player can choose;
/// any other problem lets the game start (the game page shows it).
pub async fn before_launch(app: &AppHandle, port_id: &str) -> Result<(), CommandError> {
    if !app.state::<AppState>().saves.enabled() {
        return Ok(());
    }
    match tokio::time::timeout(BEFORE_LAUNCH_LIMIT, sync(app, port_id)).await {
        Ok(Ok(saves)) if saves.status == SaveStatus::Conflict => Err(CommandError {
            code: ErrorCode::SavesConflict,
            message: "This game has different saves on another device. Choose which to keep."
                .to_owned(),
        }),
        Ok(Ok(_)) | Ok(Err(_)) => Ok(()),
        Err(_) => {
            eprintln!("syncing saves for {port_id} took too long; starting anyway");
            Ok(())
        }
    }
}

/// Syncs after a game exits, in the background.
pub fn spawn_after_exit(app: &AppHandle, port_id: &str) {
    let app = app.clone();
    let port_id = port_id.to_owned();
    tauri::async_runtime::spawn(async move { after_exit(&app, &port_id).await });
}

/// Syncs a game's saves after it exits, when cloud saves are on.
pub async fn after_exit(app: &AppHandle, port_id: &str) {
    if !app.state::<AppState>().saves.enabled() {
        return;
    }
    if let Err(e) = sync(app, port_id).await {
        eprintln!("could not sync saves for {port_id}: {}", e.message);
    }
}

/// Opens the vault in the background at start-up, so the first sync is quick.
pub fn spawn_open(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let handle = app.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let state = handle.state::<AppState>();
            if state.saves.enabled()
                && let Err(e) = state.saves.vault()
            {
                eprintln!("cloud saves aren't available yet: {e}");
            }
        })
        .await;
    });
}

/// Runs blocking cloud-saves work off the async runtime.
pub async fn blocking<T: Send + 'static>(
    app: &AppHandle,
    job: impl FnOnce(&Saves) -> Result<T, SyncError> + Send + 'static,
) -> Result<T, CommandError> {
    let handle = app.clone();
    tokio::task::spawn_blocking(move || job(&handle.state::<AppState>().saves))
        .await
        .map_err(blocking_error)?
        .map_err(CommandError::from)
}
