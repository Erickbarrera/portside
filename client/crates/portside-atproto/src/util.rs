use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

use crate::error::{AtprotoError, Result};

pub(crate) fn b64(bytes: impl AsRef<[u8]>) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub(crate) fn b64_decode(s: &str) -> Result<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|e| AtprotoError::Decode(format!("bad base64: {e}")))
}

pub(crate) fn random_bytes<const N: usize>() -> Result<[u8; N]> {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf)
        .map_err(|e| AtprotoError::Io(std::io::Error::other(format!("no randomness: {e}"))))?;
    Ok(buf)
}

/// A random URL-safe token with `N` bytes of entropy.
pub(crate) fn random_token<const N: usize>() -> Result<String> {
    Ok(b64(random_bytes::<N>()?))
}

pub(crate) fn sha256_b64(s: &str) -> String {
    b64(Sha256::digest(s.as_bytes()))
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `scheme://host[:port]`, the key DPoP nonces are kept under.
pub(crate) fn origin(url: &url::Url) -> String {
    url.origin().ascii_serialization()
}
