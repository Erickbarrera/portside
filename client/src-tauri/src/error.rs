use portside_api::ApiError;
use portside_atproto::AtprotoError;
use portside_catalog::CatalogError;
use portside_installer::InstallError;
use portside_launcher::LaunchError;
use portside_social::SocialError;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Error codes the UI can rely on (client/docs/architecture.md, "Error handling").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum ErrorCode {
    /// The API could not be reached.
    #[serde(rename = "E_API_UNREACHABLE")]
    ApiUnreachable,
    /// The thing asked for does not exist.
    #[serde(rename = "E_NOT_FOUND")]
    NotFound,
    /// The API answered with an error.
    #[serde(rename = "E_API_STATUS")]
    ApiStatus,
    /// The API answered with something the client does not understand.
    #[serde(rename = "E_API_DECODE")]
    ApiDecode,
    /// The local catalog database failed.
    #[serde(rename = "E_CATALOG_DB")]
    CatalogDb,
    /// The port has no manifest, so Portside doesn't know how to install it.
    #[serde(rename = "E_NO_MANIFEST")]
    NoManifest,
    /// The manifest needs a newer Portside.
    #[serde(rename = "E_MANIFEST_SCHEMA")]
    ManifestSchema,
    /// No build for this OS and architecture.
    #[serde(rename = "E_NO_BUILD")]
    NoBuild,
    /// The latest release doesn't have the file the manifest describes.
    #[serde(rename = "E_RELEASE_MISMATCH")]
    ReleaseMismatch,
    /// GitHub's hourly limit for anonymous requests was reached.
    #[serde(rename = "E_GITHUB_RATE_LIMIT")]
    GithubRateLimit,
    /// GitHub answered with an error.
    #[serde(rename = "E_GITHUB_STATUS")]
    GithubStatus,
    /// A download failed or was cut short.
    #[serde(rename = "E_DOWNLOAD_HTTP")]
    DownloadHttp,
    /// A download pointed somewhere other than GitHub.
    #[serde(rename = "E_HOST_NOT_ALLOWED")]
    HostNotAllowed,
    /// The downloaded file doesn't match its published checksum.
    #[serde(rename = "E_ASSET_HASH_MISMATCH")]
    AssetHashMismatch,
    /// The manifest promises a checksum the release doesn't publish.
    #[serde(rename = "E_CHECKSUM_MISSING")]
    ChecksumMissing,
    /// The download couldn't be unpacked, or didn't contain what the manifest says.
    #[serde(rename = "E_UNPACK")]
    Unpack,
    /// An unsigned macOS app needs the user's OK first.
    #[serde(rename = "E_NEEDS_CONSENT")]
    NeedsConsent,
    /// The user cancelled.
    #[serde(rename = "E_CANCELLED")]
    Cancelled,
    /// The port isn't installed.
    #[serde(rename = "E_NOT_INSTALLED")]
    NotInstalled,
    /// This port is already being installed.
    #[serde(rename = "E_INSTALL_RUNNING")]
    InstallRunning,
    /// Files or the library database on this device failed.
    #[serde(rename = "E_LIBRARY")]
    Library,
    /// The game is running, so it can't be started again, updated or removed.
    #[serde(rename = "E_GAME_RUNNING")]
    GameRunning,
    /// The game couldn't be started.
    #[serde(rename = "E_LAUNCH")]
    Launch,
    /// An Intel-only build on Apple silicon without Rosetta installed.
    #[serde(rename = "E_NEEDS_ROSETTA")]
    NeedsRosetta,
    /// This needs a Bluesky sign-in, and there is none (or it expired).
    #[serde(rename = "E_SIGNED_OUT")]
    SignedOut,
    /// Signing in failed (unknown handle, the sign-in server refused, no answer in time).
    #[serde(rename = "E_AUTH")]
    Auth,
    /// The system keychain couldn't store or read the sign-in.
    #[serde(rename = "E_KEYCHAIN")]
    Keychain,
    /// Bluesky (or the account's host) couldn't be reached.
    #[serde(rename = "E_SOCIAL_UNREACHABLE")]
    SocialUnreachable,
    /// Bluesky refused or failed a request.
    #[serde(rename = "E_SOCIAL")]
    Social,
    /// The other person's settings don't allow this message.
    #[serde(rename = "E_CHAT_NOT_ALLOWED")]
    ChatNotAllowed,
    /// The request itself is wrong (an empty message, a title that's too long).
    #[serde(rename = "E_INVALID")]
    Invalid,
    /// Cloud saves aren't set up on this device.
    #[serde(rename = "E_SAVES_OFF")]
    SavesOff,
    /// The cloud saves folder can't be reached, or a file in it isn't downloaded yet.
    #[serde(rename = "E_SAVES_UNAVAILABLE")]
    SavesUnavailable,
    /// Two devices have different saves for this game; the player has to keep one.
    #[serde(rename = "E_SAVES_CONFLICT")]
    SavesConflict,
    /// Syncing saves failed (a file couldn't be read or written, damaged data).
    #[serde(rename = "E_SAVES")]
    Saves,
    /// Steam isn't installed here, or no one has signed in to it.
    #[serde(rename = "E_STEAM_NOT_FOUND")]
    SteamNotFound,
    /// Steam is open, and has to be closed to add a game.
    #[serde(rename = "E_STEAM_RUNNING")]
    SteamRunning,
    /// Adding to Steam failed (its files couldn't be read or written).
    #[serde(rename = "E_STEAM")]
    Steam,
    /// The game is on storage that isn't there: the Steam Deck's SD card was taken out.
    #[serde(rename = "E_STORAGE_MISSING")]
    StorageMissing,
    /// The game only has a Windows build, and Proton isn't installed in Steam to run it.
    #[serde(rename = "E_NO_PROTON")]
    NoProton,
    /// Portside couldn't update itself (the update's signature didn't match, installing failed).
    #[serde(rename = "E_APP_UPDATE")]
    AppUpdate,
}

/// What a failed command returns to the UI.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CommandError {
    pub code: ErrorCode,
    /// Plain-language message, safe to show.
    pub message: String,
}

impl From<ApiError> for CommandError {
    fn from(e: ApiError) -> Self {
        let (code, message) = match &e {
            ApiError::Network(_) => (
                ErrorCode::ApiUnreachable,
                "Can't reach the Portside catalog. Check your connection and try again.".to_owned(),
            ),
            ApiError::Status { status: 404, .. } => (ErrorCode::NotFound, "Not found.".to_owned()),
            ApiError::Status { .. } => (ErrorCode::ApiStatus, e.to_string()),
            ApiError::Decode(_) | ApiError::BaseUrl(_) => (ErrorCode::ApiDecode, e.to_string()),
        };
        CommandError { code, message }
    }
}

impl From<CatalogError> for CommandError {
    fn from(e: CatalogError) -> Self {
        match e {
            CatalogError::Api(api) => api.into(),
            other => CommandError {
                code: ErrorCode::CatalogDb,
                message: format!("The saved catalog on this device has a problem: {other}"),
            },
        }
    }
}

impl From<InstallError> for CommandError {
    fn from(e: InstallError) -> Self {
        use InstallError as E;
        let code = match &e {
            E::NoManifest => ErrorCode::NoManifest,
            E::UnsupportedSchema(_) | E::UnsupportedSource(_) => ErrorCode::ManifestSchema,
            E::NoBuild { .. } => ErrorCode::NoBuild,
            E::NoRelease { .. }
            | E::AssetMissing { .. }
            | E::AssetAmbiguous { .. }
            | E::BadPattern { .. } => ErrorCode::ReleaseMismatch,
            E::RateLimited { .. } => ErrorCode::GithubRateLimit,
            E::GithubStatus { .. } => ErrorCode::GithubStatus,
            E::Network(_) | E::DownloadStatus(_) | E::SizeMismatch { .. } => {
                ErrorCode::DownloadHttp
            }
            E::HostNotAllowed(_) => ErrorCode::HostNotAllowed,
            E::HashMismatch { .. } => ErrorCode::AssetHashMismatch,
            E::ChecksumMissing(_) => ErrorCode::ChecksumMissing,
            E::UnsupportedArchive(_)
            | E::BadArchive(_)
            | E::NestedArchive { .. }
            | E::UnsafePath(_)
            | E::LaunchTargetMissing(_)
            | E::Tool { .. } => ErrorCode::Unpack,
            E::NeedsConsent => ErrorCode::NeedsConsent,
            E::Cancelled => ErrorCode::Cancelled,
            E::NotInstalled(_) => ErrorCode::NotInstalled,
            E::StorageMissing(_) => ErrorCode::StorageMissing,
            E::Io { .. } | E::Db(_) | E::Migration(_) | E::Json(_) => ErrorCode::Library,
        };
        let message = match &e {
            E::StorageMissing(_) => {
                "This game is on the SD card, and it isn't in. Put it back in, then try again."
                    .to_owned()
            }
            E::Network(_) => {
                "Can't reach GitHub to download this. Check your connection and try again."
                    .to_owned()
            }
            other => {
                let mut m = other.to_string();
                if let Some(first) = m.get(..1) {
                    m.replace_range(..1, &first.to_uppercase());
                }
                m
            }
        };
        CommandError { code, message }
    }
}

impl From<LaunchError> for CommandError {
    fn from(e: LaunchError) -> Self {
        match e {
            LaunchError::NeedsRosetta => CommandError {
                code: ErrorCode::NeedsRosetta,
                message: "This build is for Intel Macs and needs Rosetta. Install it by running \
                          `softwareupdate --install-rosetta` in Terminal, then try again."
                    .to_owned(),
            },
            other => CommandError {
                code: ErrorCode::Launch,
                message: format!("Couldn't start the game: {other}."),
            },
        }
    }
}

impl From<AtprotoError> for CommandError {
    fn from(e: AtprotoError) -> Self {
        use AtprotoError as E;
        let (code, message) = match &e {
            E::Network { .. } => (
                ErrorCode::SocialUnreachable,
                "Can't reach Bluesky. Check your connection and try again.".to_owned(),
            ),
            E::Resolve(m) => (ErrorCode::Auth, m.clone()),
            E::OAuth { .. } | E::Io(_) => (ErrorCode::Auth, format!("Sign-in failed: {e}.")),
            E::Denied => (ErrorCode::Cancelled, "Sign-in was cancelled.".to_owned()),
            E::Timeout => (
                ErrorCode::Auth,
                "Sign-in wasn't finished in the browser in time. Try again.".to_owned(),
            ),
            E::SessionExpired => (
                ErrorCode::SignedOut,
                "Your Bluesky sign-in has expired. Sign in again.".to_owned(),
            ),
            E::Store(m) => (
                ErrorCode::Keychain,
                format!("Couldn't use the system keychain: {m}"),
            ),
            E::Xrpc { error, status, .. } => {
                let chat_refusal = match error.as_deref() {
                    Some("NotFollowedBySender") => {
                        Some("They only accept messages from people they follow.")
                    }
                    Some("MessagesDisabled") => Some("They don't accept direct messages."),
                    Some("BlockedActor" | "BlockedSubject") => {
                        Some("You can't message this person.")
                    }
                    Some("RecipientNotFound") => Some("That account wasn't found."),
                    Some("AccountSuspended") => Some("Your account can't send messages right now."),
                    Some("ConvoLocked") => Some("This conversation is locked."),
                    _ => None,
                };
                match chat_refusal {
                    Some(m) => (ErrorCode::ChatNotAllowed, m.to_owned()),
                    None if *status == 404 => (ErrorCode::NotFound, "Not found.".to_owned()),
                    None => (
                        ErrorCode::Social,
                        format!(
                            "Bluesky couldn't do that: {}.",
                            e.xrpc_message()
                                .or(error.as_deref())
                                .unwrap_or("unknown error")
                        ),
                    ),
                }
            }
            E::Decode(_) => (ErrorCode::Social, format!("Bluesky sent {e}.")),
        };
        CommandError { code, message }
    }
}

impl From<portside_sync::SyncError> for CommandError {
    fn from(e: portside_sync::SyncError) -> Self {
        use portside_sync::SyncError as E;
        let code = match &e {
            E::NotEnabled => ErrorCode::SavesOff,
            E::Unavailable(_) | E::NotDownloaded(_) => ErrorCode::SavesUnavailable,
            E::NoVault | E::VaultExists | E::UnknownSnapshot | E::NoSaveFolder => {
                ErrorCode::Invalid
            }
            _ => ErrorCode::Saves,
        };
        CommandError {
            code,
            message: e.to_string(),
        }
    }
}

impl From<SocialError> for CommandError {
    fn from(e: SocialError) -> Self {
        match e {
            SocialError::Atproto(e) => e.into(),
            SocialError::Invalid(message) => CommandError {
                code: ErrorCode::Invalid,
                message,
            },
        }
    }
}
