//! Updating Portside itself (docs/build-release.md, "Auto-update").
//!
//! Releases are GitHub releases of this repository. Each one carries a `latest.json` written
//! by tauri-action, with the download and Ed25519 signature for every platform. GitHub's
//! "latest release" link skips pre-releases, and every 0.x release is one, so the newest
//! published release is looked up through the GitHub API and its `latest.json` handed to
//! `tauri-plugin-updater`, which checks the version, downloads, verifies the signature and
//! installs. The public key is baked in at build time (`plugins.updater.pubkey`, set by the
//! release workflow); a build without one can't update itself.

use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use tauri_specta::Event;

use crate::error::{CommandError, ErrorCode};
use crate::state::AppState;

/// The repository whose releases are Portside's own builds.
const REPO: &str = match option_env!("PORTSIDE_UPDATE_REPO") {
    Some(repo) => repo,
    None => "Erickbarrera/portside",
};
/// Release tags of the app (the repository also tags other things).
const TAG_PREFIX: &str = "client-v";
/// The update manifest tauri-action attaches to each release.
const MANIFEST: &str = "latest.json";
/// First check soon after start (not during start-up itself), then every few hours.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(10);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// A newer Portside that can be installed.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AvailableUpdate {
    pub version: String,
    /// "What's new", from the release notes.
    pub notes: Option<String>,
}

/// How far the download of an update has got.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateProgress {
    /// Bytes downloaded (f64: TypeScript numbers).
    pub downloaded: f64,
    /// Size of the download, when the server says.
    pub total: Option<f64>,
}

/// Whether this copy can update itself, and what the last check found.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AppUpdateStatus {
    /// Set when this copy can't update itself, saying why (a development build, a build
    /// without the update key). Nothing else here is used then.
    pub off_reason: Option<String>,
    pub checking: bool,
    /// Unix seconds of the last check that reached GitHub.
    pub last_checked_at: Option<f64>,
    pub available: Option<AvailableUpdate>,
    /// Set while an update downloads and installs; Portside restarts when it's done.
    pub progress: Option<UpdateProgress>,
    /// Set when the last check or install failed.
    pub last_error: Option<CommandError>,
}

/// Sent whenever the status changes.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct AppUpdateChanged {
    pub status: AppUpdateStatus,
}

pub struct AppUpdater {
    status: Mutex<AppUpdateStatus>,
    /// The update the last check found, kept for installing it.
    found: Mutex<Option<Update>>,
    /// Held while checking or installing, so only one runs at a time.
    busy: tokio::sync::Mutex<()>,
}

impl AppUpdater {
    pub fn new(app: &AppHandle) -> Self {
        Self {
            status: Mutex::new(AppUpdateStatus {
                off_reason: off_reason(app),
                checking: false,
                last_checked_at: None,
                available: None,
                progress: None,
                last_error: None,
            }),
            found: Mutex::new(None),
            busy: tokio::sync::Mutex::new(()),
        }
    }

    pub fn status(&self) -> AppUpdateStatus {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn change(&self, app: &AppHandle, f: impl FnOnce(&mut AppUpdateStatus)) -> AppUpdateStatus {
        let status = {
            let mut s = self.status.lock().unwrap_or_else(PoisonError::into_inner);
            f(&mut s);
            s.clone()
        };
        if let Err(e) = (AppUpdateChanged {
            status: status.clone(),
        })
        .emit(app)
        {
            eprintln!("could not notify the UI of an app update change: {e}");
        }
        status
    }
}

/// Why this copy can't update itself, if it can't.
fn off_reason(app: &AppHandle) -> Option<String> {
    if cfg!(target_os = "android") {
        return Some(
            "Portside doesn't update itself on Android yet. Download new versions from the website."
                .to_owned(),
        );
    }
    if cfg!(debug_assertions) || tauri::utils::platform::bundle_type().is_none() {
        return Some(
            "This copy of Portside wasn't installed from a release, so it doesn't update itself."
                .to_owned(),
        );
    }
    let pubkey = app
        .config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .unwrap_or_default();
    if pubkey.trim().is_empty() {
        return Some(
            "This build of Portside can't update itself. Download new versions from the website."
                .to_owned(),
        );
    }
    None
}

/// Checks for a newer Portside and records what it found.
pub async fn check(app: &AppHandle) -> AppUpdateStatus {
    let updater = app.state::<AppUpdater>();
    if updater.status().off_reason.is_some() {
        return updater.status();
    }
    let _busy = updater.busy.lock().await;
    if updater.status().progress.is_some() {
        return updater.status();
    }
    updater.change(app, |s| s.checking = true);
    let result = find_update(app).await;
    let now = now_secs();
    match result {
        Ok(update) => {
            let available = update.as_ref().map(|u| AvailableUpdate {
                version: u.version.clone(),
                notes: u.body.clone().filter(|b| !b.trim().is_empty()),
            });
            *updater.found.lock().unwrap_or_else(PoisonError::into_inner) = update;
            updater.change(app, |s| {
                s.checking = false;
                s.last_checked_at = Some(now);
                s.available = available;
                s.last_error = None;
            })
        }
        Err(e) => updater.change(app, |s| {
            s.checking = false;
            s.last_error = Some(e);
        }),
    }
}

/// Downloads and installs the update the last check found, then restarts Portside.
/// Refuses while a game runs or a game installs: restarting would cut them off.
pub async fn install(app: &AppHandle) -> Result<(), CommandError> {
    let state = app.state::<AppState>();
    if !state.games.running().is_empty() {
        return Err(CommandError {
            code: ErrorCode::GameRunning,
            message: "Quit your game first: Portside restarts to finish updating.".to_owned(),
        });
    }
    if !state.installs.active().is_empty() {
        return Err(CommandError {
            code: ErrorCode::InstallRunning,
            message: "Wait for the install to finish: Portside restarts to finish updating."
                .to_owned(),
        });
    }
    let updater = app.state::<AppUpdater>();
    let _busy = updater.busy.lock().await;
    let Some(update) = updater
        .found
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
    else {
        return Err(CommandError {
            code: ErrorCode::NotFound,
            message: "There's no update to install. Check for updates first.".to_owned(),
        });
    };
    updater.change(app, |s| {
        s.progress = Some(UpdateProgress {
            downloaded: 0.0,
            total: None,
        });
        s.last_error = None;
    });
    let mut downloaded = 0u64;
    let mut last_percent = None;
    let result = update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                // One event per percent is plenty for a progress bar.
                let percent = total.map(|t| downloaded * 100 / t.max(1));
                if percent.is_none() || percent != last_percent {
                    last_percent = percent;
                    updater.change(app, |s| {
                        s.progress = Some(UpdateProgress {
                            downloaded: downloaded as f64,
                            total: total.map(|t| t as f64),
                        });
                    });
                }
            },
            || {},
        )
        .await;
    match result {
        // On Windows the installer has already closed Portside by now.
        Ok(()) => app.restart(),
        Err(e) => {
            let error = update_error(&e);
            updater.change(app, |s| {
                s.progress = None;
                s.last_error = Some(error.clone());
            });
            Err(error)
        }
    }
}

/// Checks on start and every few hours while the app is open.
pub fn spawn_checks(app: AppHandle) {
    if app.state::<AppUpdater>().status().off_reason.is_some() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_AFTER).await;
        loop {
            // Errors are recorded in the status; the loop keeps going.
            check(&app).await;
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

async fn find_update(app: &AppHandle) -> Result<Option<Update>, CommandError> {
    let Some(manifest) = newest_manifest().await? else {
        return Ok(None);
    };
    let url = manifest.parse().map_err(|_| CommandError {
        code: ErrorCode::GithubStatus,
        message: "GitHub listed an update at an address Portside can't read.".to_owned(),
    })?;
    app.updater_builder()
        .endpoints(vec![url])
        .and_then(|b| b.build())
        .map_err(|e| update_error(&e))?
        .check()
        .await
        .map_err(|e| update_error(&e))
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

/// Address of the update manifest of the newest published app release, pre-releases included.
async fn newest_manifest() -> Result<Option<String>, CommandError> {
    let response = reqwest::Client::new()
        .get(format!(
            "https://api.github.com/repos/{REPO}/releases?per_page=20"
        ))
        .header(reqwest::header::USER_AGENT, "Portside")
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|_| CommandError {
            code: ErrorCode::ApiUnreachable,
            message: "Can't reach GitHub to check for updates. Check your connection.".to_owned(),
        })?;
    let status = response.status();
    if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::TOO_MANY_REQUESTS
    {
        return Err(CommandError {
            code: ErrorCode::GithubRateLimit,
            message: "GitHub asked Portside to slow down. Updates are checked again later."
                .to_owned(),
        });
    }
    if !status.is_success() {
        return Err(CommandError {
            code: ErrorCode::GithubStatus,
            message: format!("GitHub couldn't list Portside's releases ({status})."),
        });
    }
    let releases: Vec<GithubRelease> = response.json().await.map_err(|_| CommandError {
        code: ErrorCode::GithubStatus,
        message: "GitHub's list of Portside releases couldn't be read.".to_owned(),
    })?;
    Ok(pick_manifest(&releases))
}

/// The manifest of the first (newest) published app release that has one. Releases built
/// before the updater existed have none and are skipped.
fn pick_manifest(releases: &[GithubRelease]) -> Option<String> {
    releases
        .iter()
        .filter(|r| !r.draft && r.tag_name.starts_with(TAG_PREFIX))
        .find_map(|r| {
            r.assets
                .iter()
                .find(|a| a.name == MANIFEST)
                .map(|a| a.browser_download_url.clone())
        })
}

fn update_error(e: &tauri_plugin_updater::Error) -> CommandError {
    use tauri_plugin_updater::Error as E;
    match e {
        E::Reqwest(_) | E::Network(_) => CommandError {
            code: ErrorCode::DownloadHttp,
            message: "The update couldn't be downloaded. Check your connection and try again."
                .to_owned(),
        },
        E::Minisign(_)
        | E::Base64(_)
        | E::SignatureUtf8(_)
        | E::SignedVersionMismatch { .. }
        | E::MissingSignedVersion => CommandError {
            code: ErrorCode::AppUpdate,
            message: "The update's signature didn't match, so it wasn't installed.".to_owned(),
        },
        other => CommandError {
            code: ErrorCode::AppUpdate,
            message: format!("Portside couldn't update itself: {other}"),
        },
    }
}

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as f64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, draft: bool, assets: &[&str]) -> GithubRelease {
        GithubRelease {
            tag_name: tag.to_owned(),
            draft,
            assets: assets
                .iter()
                .map(|name| GithubAsset {
                    name: (*name).to_owned(),
                    browser_download_url: format!("https://example.invalid/{tag}/{name}"),
                })
                .collect(),
        }
    }

    #[test]
    fn picks_the_newest_published_release_with_a_manifest() {
        let releases = [
            release("client-v0.4.0", true, &["latest.json"]),
            release("other-v9", false, &["latest.json"]),
            release("client-v0.3.0", false, &["Portside.dmg", "latest.json"]),
            release("client-v0.2.0", false, &["latest.json"]),
        ];
        assert_eq!(
            pick_manifest(&releases).as_deref(),
            Some("https://example.invalid/client-v0.3.0/latest.json")
        );
    }

    #[test]
    fn skips_releases_from_before_the_updater() {
        let releases = [
            release("client-v0.2.0", false, &["Portside.dmg"]),
            release("client-v0.1.0", false, &[]),
        ];
        assert_eq!(pick_manifest(&releases), None);
    }

    #[test]
    fn reads_github_release_json() {
        let json = r#"[{"tag_name":"client-v0.3.0","draft":false,"prerelease":true,
            "assets":[{"name":"latest.json","browser_download_url":"https://github.com/x/y/releases/download/client-v0.3.0/latest.json","size":1}]}]"#;
        let releases: Vec<GithubRelease> = serde_json::from_str(json).expect("parse");
        assert_eq!(
            pick_manifest(&releases).as_deref(),
            Some("https://github.com/x/y/releases/download/client-v0.3.0/latest.json")
        );
    }
}
