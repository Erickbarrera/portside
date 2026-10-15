use serde::Deserialize;
use thiserror::Error;

/// RFC 9457 problem details, as the API sends them.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Problem {
    #[serde(rename = "type", default = "about_blank")]
    pub problem_type: String,
    #[serde(default)]
    pub title: String,
    pub status: u16,
    #[serde(default)]
    pub detail: Option<String>,
}

fn about_blank() -> String {
    "about:blank".to_owned()
}

#[derive(Debug, Error)]
pub enum ApiError {
    /// The API answered with an error status.
    #[error("API returned {status}: {}", .problem.as_ref().map_or("", |p| p.detail.as_deref().unwrap_or(p.title.as_str())))]
    Status {
        status: u16,
        problem: Option<Problem>,
        request_id: Option<String>,
    },
    /// The API could not be reached (DNS, connection, timeout, TLS).
    #[error("could not reach the Portside API: {0}")]
    Network(#[source] reqwest::Error),
    /// The response did not match the contract.
    #[error("unexpected response from the Portside API: {0}")]
    Decode(String),
    #[error("invalid API base URL: {0}")]
    BaseUrl(String),
}

impl ApiError {
    pub fn status(&self) -> Option<u16> {
        match self {
            ApiError::Status { status, .. } => Some(*status),
            _ => None,
        }
    }

    pub fn is_not_found(&self) -> bool {
        self.status() == Some(404)
    }
}
