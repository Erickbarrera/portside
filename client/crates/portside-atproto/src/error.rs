use thiserror::Error;

pub type Result<T, E = AtprotoError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum AtprotoError {
    /// A server could not be reached (DNS, connection, timeout, TLS).
    #[error("could not reach {host}")]
    Network {
        host: String,
        #[source]
        source: reqwest::Error,
    },
    /// A handle, DID or account host couldn't be found or doesn't make sense.
    #[error("{0}")]
    Resolve(String),
    /// The sign-in server refused a request (RFC 6749 error response).
    #[error("the sign-in server said {error}{}", .description.as_deref().map(|d| format!(": {d}")).unwrap_or_default())]
    OAuth {
        error: String,
        description: Option<String>,
    },
    /// The person said no on the sign-in page.
    #[error("sign-in was cancelled")]
    Denied,
    /// An XRPC call failed. `error` is the lexicon error name, e.g. `InvalidRequest`.
    #[error("{nsid} failed with {status}{}", .message.as_deref().or(.error.as_deref()).map(|m| format!(": {m}")).unwrap_or_default())]
    Xrpc {
        nsid: String,
        status: u16,
        error: Option<String>,
        message: Option<String>,
    },
    /// A server answered with something that doesn't match the protocol.
    #[error("unexpected response: {0}")]
    Decode(String),
    /// The sign-in can't be renewed any more; the person has to sign in again.
    #[error("the sign-in has expired")]
    SessionExpired,
    /// No answer came back from the browser in time.
    #[error("sign-in wasn't finished in time")]
    Timeout,
    /// Keychain or other local storage failed.
    #[error("could not store the sign-in: {0}")]
    Store(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl AtprotoError {
    pub(crate) fn network(url: &url::Url, source: reqwest::Error) -> AtprotoError {
        AtprotoError::Network {
            host: url.host_str().unwrap_or("the server").to_owned(),
            source,
        }
    }

    /// The lexicon error name of a failed XRPC call.
    pub fn xrpc_error(&self) -> Option<&str> {
        match self {
            AtprotoError::Xrpc { error, .. } => error.as_deref(),
            _ => None,
        }
    }

    /// The server's own message for a failed XRPC call.
    pub fn xrpc_message(&self) -> Option<&str> {
        match self {
            AtprotoError::Xrpc { message, .. } => message.as_deref(),
            _ => None,
        }
    }
}
