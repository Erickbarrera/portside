//! The Bluesky sign-in lives in the system keychain (macOS Keychain, Windows Credential
//! Manager, Secret Service on Linux), never in a file (client/CLAUDE.md).

use portside_atproto::{AtprotoError, Session, SessionStore};

const SERVICE: &str = "gg.portside.desktop";
const ACCOUNT: &str = "atproto-session";

pub struct KeychainStore;

impl KeychainStore {
    fn entry() -> Result<keyring::Entry, AtprotoError> {
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| AtprotoError::Store(e.to_string()))
    }
}

impl SessionStore for KeychainStore {
    fn load(&self) -> Result<Option<Session>, AtprotoError> {
        match Self::entry()?.get_password() {
            Ok(json) => serde_json::from_str(&json)
                .map(Some)
                .map_err(|e| AtprotoError::Store(format!("saved sign-in is unreadable: {e}"))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AtprotoError::Store(e.to_string())),
        }
    }

    fn save(&self, session: &Session) -> Result<(), AtprotoError> {
        let json =
            serde_json::to_string(session).map_err(|e| AtprotoError::Store(e.to_string()))?;
        Self::entry()?
            .set_password(&json)
            .map_err(|e| AtprotoError::Store(e.to_string()))
    }

    fn clear(&self) -> Result<(), AtprotoError> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AtprotoError::Store(e.to_string())),
        }
    }
}
