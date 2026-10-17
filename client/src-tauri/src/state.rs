use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use anyhow::Context;
use portside_catalog::Catalog;
use portside_core::Platform;
use portside_core::paths::AppPaths;

use crate::error::CommandError;
use crate::games::Games;
use crate::installs::Installs;
use crate::playtime::Playtime;
use crate::saves::Saves;
use crate::social::Social;

/// Where the static catalog is published (docs/specs/static-catalog.md): GitHub Pages of this
/// repository. A release build can bake in another address with
/// `PORTSIDE_DEFAULT_CATALOG_URL` at compile time. [TBD: custom domain]
pub const DEFAULT_CATALOG_URL: &str = match option_env!("PORTSIDE_DEFAULT_CATALOG_URL") {
    Some(url) => url,
    None => "https://erickbarrera.github.io/portside",
};

pub struct AppState {
    pub api: portside_api::Client,
    /// Portside's own folders.
    pub paths: AppPaths,
    pub platform: Platform,
    pub catalog: Arc<Catalog>,
    pub installs: Installs,
    pub games: Games,
    pub social: Social,
    pub saves: Saves,
    /// Play time from the player's other devices.
    pub playtime: Playtime,
    /// Held while a catalog sync runs, so only one runs at a time.
    pub sync_lock: tokio::sync::Mutex<()>,
    /// Outcome of the last sync attempt, for the UI.
    pub last_sync_error: Mutex<Option<CommandError>>,
}

impl AppState {
    /// Reads `PORTSIDE_CATALOG_URL` and `PORTSIDE_DATA_DIR` (both optional).
    pub fn from_env() -> anyhow::Result<AppState> {
        let base = std::env::var("PORTSIDE_CATALOG_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_CATALOG_URL.to_owned());
        let user_agent = format!("portside-desktop/{}", env!("CARGO_PKG_VERSION"));
        let api = portside_api::Client::new(base.trim(), &user_agent)
            .with_context(|| format!("PORTSIDE_CATALOG_URL={base} is not usable"))?;

        let paths = match std::env::var_os("PORTSIDE_DATA_DIR") {
            Some(dir) => AppPaths::under(PathBuf::from(dir)),
            None => AppPaths::discover().context("finding the app data folder")?,
        };
        // One copy per catalog address, so pointing the app elsewhere never mixes catalogs.
        let file = paths
            .data_dir
            .join(format!("catalog-{}.db", file_safe(api.base_url())));
        let catalog = match Catalog::open(&file) {
            Ok(c) => c,
            Err(e) => {
                // Still usable online; the copy just won't survive a restart.
                eprintln!(
                    "could not open the catalog at {}: {e}; using memory",
                    file.display()
                );
                Catalog::open_in_memory().context("opening an in-memory catalog")?
            }
        };

        let installs = Installs::open(&paths, &user_agent).context("opening the game library")?;
        // The website is published with the catalog (ADR-0018); invites link to it.
        let social = Social::new(&user_agent, api.base_url()).context("setting up Bluesky")?;

        let saves = Saves::open(&paths).context("setting up cloud saves")?;

        Ok(AppState {
            api,
            platform: Platform::current(),
            catalog: Arc::new(catalog),
            installs,
            games: Games::new(paths.logs_dir.clone()),
            social,
            saves,
            playtime: Playtime::default(),
            sync_lock: tokio::sync::Mutex::new(()),
            last_sync_error: Mutex::new(None),
            paths,
        })
    }

    pub fn last_sync_error(&self) -> Option<CommandError> {
        self.last_sync_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn set_last_sync_error(&self, error: Option<CommandError>) {
        *self
            .last_sync_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = error;
    }
}

fn file_safe(s: &str) -> String {
    s.trim_end_matches('/')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn file_safe_names() {
        assert_eq!(
            super::file_safe("http://localhost:3000/"),
            "http___localhost_3000"
        );
    }
}
