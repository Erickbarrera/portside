//! Starts an installed port, follows it until it exits and stops it when asked
//! (client/docs/install-launch.md, "Launch").
//!
//! No Tauri here; `src-tauri` keeps the running games and records play time.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use portside_api::models::LaunchSpec;
use portside_core::types::Os;
use portside_installer::Installed;
use tokio::process::{Child, Command};
use tokio::sync::Notify;

/// How long a game gets to quit after Stop before it is killed.
pub const STOP_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("the manifest doesn't say how to start this build")]
    NoLaunchTarget,
    #[error("launch path {0:?} leaves the install folder")]
    UnsafePath(String),
    #[error("{} is missing; reinstall the game", .0.display())]
    Missing(PathBuf),
    #[error("{} has no program to run: {reason}", bundle.display())]
    BadBundle { bundle: PathBuf, reason: String },
    #[error("this build is for Intel Macs and needs Rosetta")]
    NeedsRosetta,
    #[error("{0:?} builds can't be started from here")]
    Unsupported(Os),
    #[error("could not start {}: {source}", program.display())]
    Spawn {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub type Result<T, E = LaunchError> = std::result::Result<T, E>;

/// What to run for an install: resolved paths, ready to spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Added to Portside's own environment.
    pub env: BTreeMap<String, String>,
}

/// Works out how to start `install` from its manifest build. `{install}` in args, env and
/// cwd is the install folder. A macOS `app_bundle` runs the bundle's own executable, so
/// Portside can follow and stop it.
pub fn command_for(install: &Installed) -> Result<LaunchCommand> {
    if install.os == Os::Android {
        return Err(LaunchError::Unsupported(install.os));
    }
    let dir = PathBuf::from(&install.dir);
    let launch = install
        .build
        .launch
        .as_ref()
        .ok_or(LaunchError::NoLaunchTarget)?;
    let program = match (&launch.app_bundle, &launch.exe) {
        (Some(bundle), _) if install.os == Os::Macos => {
            bundle_executable(&dir.join(safe_relative(bundle)?))?
        }
        (_, Some(exe)) => dir.join(safe_relative(exe)?),
        _ => return Err(LaunchError::NoLaunchTarget),
    };
    if !program.is_file() {
        return Err(LaunchError::Missing(program));
    }
    let expand = |s: &str| s.replace("{install}", &install.dir);
    let cwd = match launch.cwd.as_deref().map(expand) {
        Some(c) if Path::new(&c).is_absolute() => {
            let c = PathBuf::from(c);
            if !c.starts_with(&dir) {
                return Err(LaunchError::UnsafePath(c.display().to_string()));
            }
            c
        }
        Some(c) if c != "." && !c.is_empty() => dir.join(safe_relative(&c)?),
        _ => dir.clone(),
    };
    Ok(LaunchCommand {
        program,
        args: args_of(launch).iter().map(|a| expand(a)).collect(),
        cwd,
        env: launch
            .env
            .iter()
            .flatten()
            .map(|(k, v)| (k.clone(), expand(v)))
            .collect(),
    })
}

/// Where to find Proton for a Windows build run on Linux (`BuildFit::Proton`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtonSetup {
    /// Proton's `proton` script.
    pub script: PathBuf,
    /// The Steam folder Proton belongs to.
    pub steam_dir: PathBuf,
    /// This game's own prefix, where Proton keeps its Windows files.
    pub prefix: PathBuf,
}

/// The command for a Windows build run through Proton: `proton run <exe> <args>`, with the
/// prefix and Steam folder Proton needs in its environment. The prefix is made if missing.
pub fn proton_command_for(install: &Installed, proton: &ProtonSetup) -> Result<LaunchCommand> {
    let mut cmd = command_for(install)?;
    std::fs::create_dir_all(&proton.prefix).map_err(|source| LaunchError::Io {
        path: proton.prefix.clone(),
        source,
    })?;
    let exe = std::mem::replace(&mut cmd.program, proton.script.clone());
    cmd.args
        .splice(0..0, ["run".to_owned(), exe.to_string_lossy().into_owned()]);
    cmd.env.insert(
        "STEAM_COMPAT_DATA_PATH".into(),
        proton.prefix.to_string_lossy().into_owned(),
    );
    cmd.env.insert(
        "STEAM_COMPAT_CLIENT_INSTALL_PATH".into(),
        proton.steam_dir.to_string_lossy().into_owned(),
    );
    Ok(cmd)
}

fn args_of(launch: &LaunchSpec) -> &[String] {
    launch.args.as_deref().unwrap_or_default()
}

/// The program inside a `.app`: `Contents/MacOS/<CFBundleExecutable>`, or the only file in
/// `Contents/MacOS` when Info.plist doesn't name one.
pub fn bundle_executable(bundle: &Path) -> Result<PathBuf> {
    if !bundle.is_dir() {
        return Err(LaunchError::Missing(bundle.to_path_buf()));
    }
    let macos = bundle.join("Contents").join("MacOS");
    let named = plist::Value::from_file(bundle.join("Contents").join("Info.plist"))
        .ok()
        .and_then(|v| {
            v.as_dictionary()
                .and_then(|d| d.get("CFBundleExecutable"))
                .and_then(|e| e.as_string())
                .map(str::to_owned)
        });
    if let Some(name) = named {
        let rel = safe_relative(&name)?;
        if rel.components().count() != 1 {
            return Err(LaunchError::UnsafePath(name));
        }
        return Ok(macos.join(rel));
    }
    let bad = |reason: &str| LaunchError::BadBundle {
        bundle: bundle.to_path_buf(),
        reason: reason.to_owned(),
    };
    let entries = std::fs::read_dir(&macos).map_err(|_| bad("no Contents/MacOS folder"))?;
    let files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .collect();
    match <[PathBuf; 1]>::try_from(files) {
        Ok([only]) => Ok(only),
        Err(_) => Err(bad("Info.plist names no CFBundleExecutable")),
    }
}

/// A relative path with only normal components.
fn safe_relative(name: &str) -> Result<PathBuf> {
    let mut clean = PathBuf::new();
    for c in Path::new(name).components() {
        match c {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => return Err(LaunchError::UnsafePath(name.to_owned())),
        }
    }
    if clean.as_os_str().is_empty() {
        return Err(LaunchError::UnsafePath(name.to_owned()));
    }
    Ok(clean)
}

/// How a game ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exit {
    /// None when it was ended by a signal.
    pub code: Option<i32>,
    /// It ended because Portside stopped it.
    pub stopped: bool,
}

/// A game Portside started.
#[derive(Debug)]
pub struct Running {
    child: Child,
    pid: u32,
}

/// Starts `cmd`. Its output goes to `log` (replaced on each run) when given. On Unix the game
/// gets its own process group, so Stop reaches the helpers it starts too.
pub fn spawn(cmd: &LaunchCommand, log: Option<&Path>) -> Result<Running> {
    let mut command = Command::new(&cmd.program);
    command
        .args(&cmd.args)
        .current_dir(&cmd.cwd)
        .envs(&cmd.env)
        .stdin(Stdio::null());
    match log {
        Some(path) => {
            let io = |source| LaunchError::Io {
                path: path.to_path_buf(),
                source,
            };
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(io)?;
            }
            let out = std::fs::File::create(path).map_err(io)?;
            let err = out.try_clone().map_err(io)?;
            command.stdout(out).stderr(err);
        }
        None => {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }
    }
    #[cfg(unix)]
    command.process_group(0);

    let child = command.spawn().map_err(|source| spawn_error(cmd, source))?;
    let pid = child.id().ok_or_else(|| LaunchError::Spawn {
        program: cmd.program.clone(),
        source: std::io::Error::other("exited before it could be followed"),
    })?;
    Ok(Running { child, pid })
}

fn spawn_error(cmd: &LaunchCommand, source: std::io::Error) -> LaunchError {
    // EBADARCH: an Intel-only binary on Apple silicon without Rosetta.
    if cfg!(target_os = "macos") && source.raw_os_error() == Some(86) {
        return LaunchError::NeedsRosetta;
    }
    LaunchError::Spawn {
        program: cmd.program.clone(),
        source,
    }
}

impl Running {
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Waits for the game to exit. When `stop` is notified first, asks it to quit, and kills
    /// it after [`STOP_GRACE`].
    pub async fn wait(mut self, stop: &Notify) -> Result<Exit> {
        let io = |source| LaunchError::Io {
            path: PathBuf::from(format!("process {}", self.pid)),
            source,
        };
        let exited = tokio::select! {
            status = self.child.wait() => Some(status.map_err(io)?),
            () = stop.notified() => None,
        };
        if let Some(status) = exited {
            return Ok(Exit {
                code: status.code(),
                stopped: false,
            });
        }
        terminate(self.pid, false).await;
        let status = match tokio::time::timeout(STOP_GRACE, self.child.wait()).await {
            Ok(status) => status.map_err(io)?,
            Err(_) => {
                terminate(self.pid, true).await;
                // The group kill normally did it; this covers a game that left its group.
                let _ = self.child.start_kill();
                self.child.wait().await.map_err(io)?
            }
        };
        Ok(Exit {
            code: status.code(),
            stopped: true,
        })
    }
}

/// Asks the game and the processes it started to quit, or forces them.
async fn terminate(pid: u32, force: bool) {
    #[cfg(unix)]
    let result = Command::new("kill")
        .args([
            if force { "-KILL" } else { "-TERM" },
            "--",
            &format!("-{pid}"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    #[cfg(windows)]
    let result = {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let pid = pid.to_string();
        let mut args = vec!["/PID", pid.as_str(), "/T"];
        if force {
            args.push("/F");
        }
        Command::new("taskkill")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
    };
    if let Err(e) = result {
        eprintln!("could not stop process {pid}: {e}");
    }
}

#[cfg(test)]
mod tests;
