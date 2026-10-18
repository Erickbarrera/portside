//! Games published as APKs (client/docs/platforms.md, "Android"): Android installs, starts and
//! removes them, through the Kotlin plugin `AppsPlugin.kt` in `gen/android`. Portside only
//! downloads and checks the APK; the player confirms the install in Android's own screen.

use std::path::Path;

use portside_api::models::ArchiveKind;
use portside_installer::Installed;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager, Wry};

use crate::error::{CommandError, ErrorCode};
use crate::state::AppState;

/// The Kotlin side, registered when the app starts.
pub struct Apps(PluginHandle<Wry>);

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new("portside-apps")
        .setup(|app, api| {
            let handle = api.register_android_plugin("gg.portside.app", "AppsPlugin")?;
            app.manage(Apps(handle));
            Ok(())
        })
        .build()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallArgs<'a> {
    path: &'a str,
    package_name: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageArgs<'a> {
    package_name: &'a str,
}

#[derive(Deserialize)]
struct InstalledReply {
    installed: bool,
}

#[derive(Deserialize)]
struct UninstallReply {
    removed: bool,
}

impl Apps {
    async fn call<T: DeserializeOwned>(
        &self,
        command: &str,
        args: impl Serialize,
    ) -> Result<T, CommandError> {
        self.0
            .run_mobile_plugin_async(command, args)
            .await
            .map_err(plugin_error)
    }

    /// Hands `apk` to Android's installer and waits until the player confirmed and Android
    /// installed it (or refused).
    pub async fn install(&self, apk: &Path, package_name: &str) -> Result<(), CommandError> {
        let path = apk.to_string_lossy();
        self.call::<serde_json::Value>(
            "install",
            InstallArgs {
                path: &path,
                package_name,
            },
        )
        .await
        .map(|_| ())
    }

    /// Opens the game. Android runs it as its own app, so Portside doesn't follow it.
    pub async fn launch(&self, package_name: &str) -> Result<(), CommandError> {
        self.call::<serde_json::Value>("launch", PackageArgs { package_name })
            .await
            .map(|_| ())
    }

    pub async fn is_installed(&self, package_name: &str) -> Result<bool, CommandError> {
        self.call::<InstalledReply>("installed", PackageArgs { package_name })
            .await
            .map(|r| r.installed)
    }

    /// Asks Android to remove the game; false when the player said no.
    pub async fn uninstall(&self, package_name: &str) -> Result<bool, CommandError> {
        self.call::<UninstallReply>("uninstall", PackageArgs { package_name })
            .await
            .map(|r| r.removed)
    }
}

fn plugin_error(e: tauri::plugin::mobile::PluginInvokeError) -> CommandError {
    use tauri::plugin::mobile::PluginInvokeError as E;
    match e {
        E::InvokeRejected(r) => {
            let code = match r.code.as_deref() {
                Some("E_CANCELLED") => ErrorCode::Cancelled,
                Some("E_NOT_INSTALLED") => ErrorCode::NotInstalled,
                Some("E_INSTALL_PERMISSION") => ErrorCode::InstallPermission,
                _ => ErrorCode::AndroidInstall,
            };
            CommandError {
                code,
                message: r
                    .message
                    .unwrap_or_else(|| "Android couldn't do that.".to_owned()),
            }
        }
        other => CommandError {
            code: ErrorCode::AndroidInstall,
            message: format!("Portside couldn't reach Android: {other}"),
        },
    }
}

/// Android's half of installing an APK build, once Portside has downloaded and checked it:
/// the player confirms in Android's screen, then the APK is deleted (Android keeps its own
/// copy). When Android doesn't install it, the library goes back to `before`.
pub async fn finish_install(
    app: &tauri::AppHandle,
    install: Installed,
    before: Option<Installed>,
) -> Result<Installed, CommandError> {
    if install.build.archive != ArchiveKind::Apk {
        return Ok(install);
    }
    let apk = Path::new(&install.dir).join(&install.asset);
    let installed = match install.build.android_package.as_deref() {
        Some(package) => app.state::<Apps>().install(&apk, package).await,
        None => Err(CommandError {
            code: ErrorCode::NoManifest,
            message: "The manifest doesn't name this game's Android package.".to_owned(),
        }),
    };
    let state = app.state::<AppState>();
    let installer = state.installs.installer();
    match installed {
        Ok(()) => {
            if let Err(e) = tokio::fs::remove_file(&apk).await {
                eprintln!("could not delete {}: {e}", apk.display());
            }
            Ok(install)
        }
        Err(e) => {
            let restored = match &before {
                Some(before) => installer.library().add(before).map_err(CommandError::from),
                None => installer
                    .uninstall(&install.port_id)
                    .await
                    .map_err(CommandError::from),
            };
            if let Err(r) = restored {
                eprintln!(
                    "could not undo the install of {}: {}",
                    install.port_id, r.message
                );
            }
            Err(e)
        }
    }
}

/// The Android package of an installed APK build.
fn package_of(install: &Installed) -> Option<&str> {
    match install.build.archive {
        ArchiveKind::Apk => install.build.android_package.as_deref(),
        _ => None,
    }
}

/// Opens an installed game.
pub async fn launch(app: &tauri::AppHandle, port_id: &str) -> Result<(), CommandError> {
    let state = app.state::<AppState>();
    let installer = state.installs.installer();
    let install = installer.library().current(port_id)?.ok_or_else(|| {
        CommandError::from(portside_installer::InstallError::NotInstalled(
            port_id.to_owned(),
        ))
    })?;
    let package = package_of(&install).ok_or_else(|| CommandError {
        code: ErrorCode::Launch,
        message: "This build can't be started on Android.".to_owned(),
    })?;
    app.state::<Apps>().launch(package).await
}

/// Before forgetting a game: asks Android to remove it. Fails with `E_CANCELLED` when the
/// player keeps it.
pub async fn before_uninstall(app: &tauri::AppHandle, port_id: &str) -> Result<(), CommandError> {
    let state = app.state::<AppState>();
    let installer = state.installs.installer();
    let Some(install) = installer.library().current(port_id)? else {
        return Ok(());
    };
    let Some(package) = package_of(&install) else {
        return Ok(());
    };
    if app.state::<Apps>().uninstall(package).await? {
        Ok(())
    } else {
        Err(CommandError {
            code: ErrorCode::Cancelled,
            message: "The game is still installed.".to_owned(),
        })
    }
}

/// Forgets games the player removed in Android's settings, so the library matches the device.
pub async fn forget_removed(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let installer = state.installs.installer();
    let Ok(all) = installer.library().list() else {
        return;
    };
    for install in all {
        let Some(package) = package_of(&install) else {
            continue;
        };
        // An install in progress is in the library before Android has it.
        if state.installs.is_running(&install.port_id) {
            continue;
        }
        if let Ok(false) = app.state::<Apps>().is_installed(package).await
            && let Err(e) = installer.uninstall(&install.port_id).await
        {
            eprintln!("could not forget {}: {e}", install.port_id);
        }
    }
}
