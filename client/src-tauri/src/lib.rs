//! Portside desktop app: wires the feature crates to the webview through typed commands.

mod app_update;
mod catalog_sync;
mod commands;
mod error;
mod gamepad;
mod games;
mod installs;
mod keychain;
mod playtime;
mod proton;
mod saves;
mod social;
mod state;
mod steam;
mod storage;

use tauri_specta::{Builder, collect_commands, collect_events};

/// Where generated TypeScript bindings go (client/CLAUDE.md: never hand-write IPC types).
pub const BINDINGS_PATH: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../ui/src/lib/ipc/bindings.ts");

/// The command/event registry shared by the app and the bindings exporter.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::app::app_info,
            commands::app::app_update_status,
            commands::app::app_update_check,
            commands::app::app_update_install,
            commands::catalog::catalog_search,
            commands::catalog::port_get,
            commands::catalog::catalog_summaries,
            commands::catalog::catalog_status,
            commands::catalog::catalog_refresh,
            commands::install::install_plan,
            commands::install::install_start,
            commands::install::install_storage,
            commands::install::install_cancel,
            commands::install::installs_active,
            commands::install::library_list,
            commands::install::library_get,
            commands::install::uninstall,
            commands::install::install_reveal,
            commands::game::game_launch,
            commands::game::game_stop,
            commands::game::games_running,
            commands::game::play_stats,
            commands::game::game_reveal_log,
            commands::social::auth_status,
            commands::social::auth_sign_in,
            commands::social::auth_cancel,
            commands::social::auth_sign_out,
            commands::social::friends_list,
            commands::social::friends_follows_you,
            commands::social::people_search,
            commands::social::friend_add,
            commands::social::friend_remove,
            commands::social::chat_conversations,
            commands::social::chat_open,
            commands::social::chat_get,
            commands::social::chat_messages,
            commands::social::chat_send,
            commands::social::chat_mark_read,
            commands::social::chat_accept,
            commands::social::chat_watch,
            commands::social::game_invite,
            commands::social::board_threads,
            commands::social::board_thread,
            commands::social::board_post,
            commands::social::board_post_site,
            commands::social::board_reply,
            commands::saves::saves_status,
            commands::saves::saves_pick_folder,
            commands::saves::saves_use_folder,
            commands::saves::saves_turn_off,
            commands::saves::saves_devices,
            commands::saves::saves_game,
            commands::saves::saves_sync,
            commands::saves::saves_keep,
            commands::saves::saves_set_game_folder,
            commands::saves::saves_reveal,
            commands::steam::steam_status,
            commands::steam::steam_add,
            commands::steam::play_request,
            commands::steam::steam_keyboard,
        ])
        .events(collect_events![
            app_update::AppUpdateChanged,
            catalog_sync::CatalogSynced,
            installs::InstallChanged,
            games::GameChanged,
            gamepad::GamepadInput,
            social::AuthChanged,
            social::ChatChanged,
            saves::SavesChanged,
            playtime::PlaytimeChanged
        ])
        // Catalog numbers (stars, asset sizes, revisions) stay far below 2^53.
        .dangerously_cast_bigints_to_number()
}

/// Writes the TypeScript bindings for the UI.
pub fn export_bindings(path: &str) -> Result<(), String> {
    specta_builder()
        .export(specta_typescript::Typescript::default(), path)
        .map_err(|e| e.to_string())
}

pub fn run() {
    let builder = specta_builder();

    let state = match state::AppState::from_env() {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Portside could not start: {e:#}");
            std::process::exit(1);
        }
    };

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state)
        .manage(steam::PlayRequest::from_args(std::env::args().skip(1)))
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            tauri::Manager::manage(app, app_update::AppUpdater::new(app.handle()));
            app_update::spawn_checks(app.handle().clone());
            catalog_sync::spawn_refresh(app.handle().clone());
            social::spawn_restore(app.handle().clone());
            social::spawn_chat_poller(app.handle().clone());
            saves::spawn_open(app.handle().clone());
            gamepad::spawn(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("Portside stopped with an error: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    /// `cargo test -p portside-desktop` regenerates the bindings; CI fails if they changed.
    /// (Not done at app start: a debug binary run from elsewhere must not write files.)
    #[test]
    fn export_bindings() {
        super::export_bindings(super::BINDINGS_PATH).expect("export bindings");
    }
}
