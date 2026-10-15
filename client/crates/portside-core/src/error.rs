use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("could not find a home directory for this user")]
    NoHomeDir,
}
