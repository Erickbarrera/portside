//! Running Windows-only ports on Linux through Proton (client/docs/platforms.md, "Proton").

use portside_installer::{BuildFit, Installed};
use portside_launcher::{LaunchCommand, ProtonSetup};

use crate::error::{CommandError, ErrorCode};
use crate::state::AppState;

pub fn missing() -> CommandError {
    CommandError {
        code: ErrorCode::NoProton,
        message: "This game only has a Windows build, which needs Proton to run here. In Steam's \
                  library, search for Proton, install the newest version, then try again."
            .to_owned(),
    }
}

/// How to start an installed game: directly, or through Proton for a Windows build.
pub fn command_for(state: &AppState, install: &Installed) -> Result<LaunchCommand, CommandError> {
    if install.fit != BuildFit::Proton {
        return Ok(portside_launcher::command_for(install)?);
    }
    let proton = portside_steam::find_proton().ok_or_else(missing)?;
    let prefix =
        portside_core::paths::proton_prefix(&state.paths, &install.port_id).ok_or_else(|| {
            CommandError {
                code: ErrorCode::Launch,
                message: "This game's name can't be used as a folder name.".to_owned(),
            }
        })?;
    Ok(portside_launcher::proton_command_for(
        install,
        &ProtonSetup {
            script: proton.script(),
            steam_dir: proton.steam_dir,
            prefix,
        },
    )?)
}
