//! AT Protocol for Portside (ADR-0004, ADR-0016): find an account's host from its handle,
//! sign in with OAuth from the desktop (PAR + PKCE + DPoP, loopback redirect) and make
//! authenticated XRPC calls. No Tauri here; `src-tauri` keeps the session in the keychain.

mod agent;
mod dpop;
mod error;
pub mod identity;
mod loopback;
pub mod oauth;
mod session;
mod util;

pub use crate::agent::{Agent, BSKY_CHAT_PROXY};
pub use crate::dpop::DpopKey;
pub use crate::error::{AtprotoError, Result};
pub use crate::identity::{Identity, Resolver};
pub use crate::loopback::CallbackListener;
pub use crate::oauth::{CallbackParams, ClientConfig, OAuthClient, PendingSignIn};
pub use crate::session::{MemoryStore, Session, SessionStore};
