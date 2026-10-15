use serde::{Deserialize, Serialize};

use crate::error::Result;

/// A signed-in account: everything needed to call its host and renew the sign-in.
/// Secret: only ever stored in the system keychain (client/CLAUDE.md).
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub did: String,
    /// Verified handle at sign-in time, if it pointed back to the DID.
    pub handle: Option<String>,
    /// The account host (PDS), e.g. `https://morel.us-east.host.bsky.network/`.
    pub pds: String,
    /// The sign-in server that issued the tokens.
    pub issuer: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub revocation_endpoint: Option<String>,
    pub client_id: String,
    pub scope: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Unix seconds.
    pub expires_at: u64,
    /// The DPoP private key the tokens are bound to.
    pub dpop_key: String,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("did", &self.did)
            .field("handle", &self.handle)
            .field("pds", &self.pds)
            .field("issuer", &self.issuer)
            .field("scope", &self.scope)
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

impl Session {
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scope.split_whitespace().any(|s| s == scope)
    }
}

/// Where the current sign-in is kept between runs. The desktop app uses the system
/// keychain; tests use [`MemoryStore`].
pub trait SessionStore: Send + Sync {
    fn load(&self) -> Result<Option<Session>>;
    fn save(&self, session: &Session) -> Result<()>;
    fn clear(&self) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct MemoryStore(std::sync::Mutex<Option<Session>>);

impl SessionStore for MemoryStore {
    fn load(&self) -> Result<Option<Session>> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    fn save(&self, session: &Session) -> Result<()> {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(session.clone());
        Ok(())
    }

    fn clear(&self) -> Result<()> {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        Ok(())
    }
}
