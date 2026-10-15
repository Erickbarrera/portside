use portside_atproto::AtprotoError;
use thiserror::Error;

pub type Result<T, E = SocialError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum SocialError {
    #[error(transparent)]
    Atproto(#[from] AtprotoError),
    /// Input the app should have caught (empty text, a bad id).
    #[error("{0}")]
    Invalid(String),
}

impl SocialError {
    /// The lexicon error name of a failed call, e.g. `NotFollowedBySender`.
    pub fn xrpc_error(&self) -> Option<&str> {
        match self {
            SocialError::Atproto(e) => e.xrpc_error(),
            SocialError::Invalid(_) => None,
        }
    }
}
