//! Runs installs in the background and reports them to the UI as `install-changed` events.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use portside_core::Platform;
use portside_core::paths::AppPaths;
use portside_installer::{
    GITHUB_API, HostPolicy, InstallPlan, Installed, Installer, InstallerConfig, Library, Progress,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::error::{CommandError, ErrorCode};
use crate::state::AppState;

/// A plan the user saw stays good this long; after that `install_start` asks again.
const PLAN_FRESH_FOR: Duration = Duration::from_secs(10 * 60);
/// Download progress events per second, at most.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

/// Where one install has got to.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum InstallState {
    Downloading { done: u64, total: Option<u64> },
    Verifying,
    Unpacking,
    Installed { install: Box<Installed> },
    Failed { error: CommandError },
    Cancelled,
    Uninstalled,
}

/// Sent whenever an install moves on, finishes, fails or is removed.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct InstallChanged {
    pub port_id: String,
    pub state: InstallState,
}

struct Job {
    cancel: Arc<AtomicBool>,
    state: InstallState,
}

pub struct Installs {
    installer: Arc<Installer>,
    plans: Mutex<HashMap<String, (Instant, InstallPlan)>>,
    jobs: Mutex<HashMap<String, Job>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Installs {
    pub fn open(
        paths: &AppPaths,
        user_agent: &str,
    ) -> Result<Installs, portside_installer::InstallError> {
        let file = paths.data_dir.join("library.db");
        let library = match Library::open(&file) {
            Ok(l) => l,
            Err(e) => {
                // Installs still work this session; they just won't be remembered.
                eprintln!(
                    "could not open the library at {}: {e}; using memory",
                    file.display()
                );
                Library::open_in_memory()?
            }
        };
        let installer = Installer::new(
            InstallerConfig {
                library_dir: paths.library_dir.clone(),
                platform: Platform::current(),
                user_agent: user_agent.to_owned(),
                github_api: GITHUB_API.to_owned(),
                hosts: HostPolicy::github(),
            },
            library,
        )?;
        Ok(Installs {
            installer: Arc::new(installer),
            plans: Mutex::new(HashMap::new()),
            jobs: Mutex::new(HashMap::new()),
        })
    }

    pub fn installer(&self) -> &Installer {
        &self.installer
    }

    pub fn remember_plan(&self, plan: InstallPlan) {
        lock(&self.plans).insert(plan.summary.port_id.clone(), (Instant::now(), plan));
    }

    fn fresh_plan(&self, port_id: &str) -> Option<InstallPlan> {
        lock(&self.plans)
            .get(port_id)
            .filter(|(at, _)| at.elapsed() < PLAN_FRESH_FOR)
            .map(|(_, p)| p.clone())
    }

    /// Installs that are running now, with where each has got to.
    pub fn active(&self) -> Vec<InstallChanged> {
        lock(&self.jobs)
            .iter()
            .map(|(port_id, job)| InstallChanged {
                port_id: port_id.clone(),
                state: job.state.clone(),
            })
            .collect()
    }

    pub fn is_running(&self, port_id: &str) -> bool {
        lock(&self.jobs).contains_key(port_id)
    }

    pub fn cancel(&self, port_id: &str) -> bool {
        match lock(&self.jobs).get(port_id) {
            Some(job) => {
                job.cancel.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }
}

fn emit(app: &AppHandle, port_id: &str, state: InstallState) {
    let event = InstallChanged {
        port_id: port_id.to_owned(),
        state: state.clone(),
    };
    {
        let installs = &app.state::<AppState>().installs;
        if let Some(job) = lock(&installs.jobs).get_mut(port_id) {
            job.state = state;
        }
    }
    if let Err(e) = event.emit(app) {
        eprintln!("could not send install progress to the UI: {e}");
    }
}

/// Starts installing `port_id` in the background. Uses the plan the user just confirmed
/// when it is still fresh; otherwise plans again.
pub async fn start(
    app: &AppHandle,
    port_id: String,
    accept_unsigned: bool,
) -> Result<(), CommandError> {
    let state = app.state::<AppState>();
    state.games.check_not_running(&port_id)?;
    let plan = match state.installs.fresh_plan(&port_id) {
        Some(p) => p,
        None => {
            let port = state.api.port(&port_id).await?;
            state.installs.installer().plan(&port).await?
        }
    };
    if plan.summary.unsigned && !accept_unsigned {
        return Err(portside_installer::InstallError::NeedsConsent.into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut jobs = lock(&state.installs.jobs);
        if jobs.contains_key(&port_id) {
            return Err(CommandError {
                code: ErrorCode::InstallRunning,
                message: "This game is already being installed.".to_owned(),
            });
        }
        jobs.insert(
            port_id.clone(),
            Job {
                cancel: cancel.clone(),
                state: InstallState::Downloading {
                    done: 0,
                    total: Some(plan.summary.size),
                },
            },
        );
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let installer = app.state::<AppState>().installs.installer.clone();
        let last = Mutex::new(Instant::now() - PROGRESS_EVERY);
        let report = |p: Progress| {
            let state = match p {
                Progress::Downloading { done, total } => {
                    let mut last = lock(&last);
                    if last.elapsed() < PROGRESS_EVERY && Some(done) != total {
                        return;
                    }
                    *last = Instant::now();
                    InstallState::Downloading { done, total }
                }
                Progress::Verifying => InstallState::Verifying,
                Progress::Unpacking => InstallState::Unpacking,
            };
            emit(&app, &plan.summary.port_id, state);
        };
        let outcome = installer
            .install(&plan, accept_unsigned, &cancel, &report)
            .await;
        let port_id = plan.summary.port_id.clone();
        let installs = &app.state::<AppState>().installs;
        lock(&installs.jobs).remove(&port_id);
        lock(&installs.plans).remove(&port_id);
        let state = match outcome {
            Ok(install) => InstallState::Installed {
                install: Box::new(install),
            },
            Err(portside_installer::InstallError::Cancelled) => InstallState::Cancelled,
            Err(e) => InstallState::Failed { error: e.into() },
        };
        emit(&app, &port_id, state);
    });
    Ok(())
}

pub fn announce_uninstall(app: &AppHandle, port_id: &str) {
    emit(app, port_id, InstallState::Uninstalled);
}
