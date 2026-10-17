use std::time::Duration;

use portside_core::{Os, PlayableStatus, PortType};
use reqwest::{StatusCode, header};
use serde::de::DeserializeOwned;
use url::Url;

use crate::error::{ApiError, Problem};
use crate::models::{CatalogSnapshot, Game, PortDetail, Release};

/// Search filters. The catalog is searched on the device (`portside-catalog`); the static
/// catalog has no search endpoint.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchParams {
    pub q: Option<String>,
    pub os: Option<Os>,
    pub status: Option<PlayableStatus>,
    pub port_type: Option<PortType>,
    pub cursor: Option<String>,
    /// 1–100, default 50.
    pub limit: Option<u32>,
}

/// Result of a conditional snapshot fetch.
#[derive(Debug, Clone, PartialEq)]
pub enum SnapshotResult {
    NotModified {
        etag: Option<String>,
    },
    Snapshot {
        etag: Option<String>,
        snapshot: CatalogSnapshot,
    },
}

/// Client for the static catalog (docs/specs/static-catalog.md): plain JSON files under
/// `v1/`, published to GitHub Pages. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Client {
    base: Url,
    http: reqwest::Client,
}

impl Client {
    /// `base_url` is where the catalog is published, e.g.
    /// `https://erickbarrera.github.io/portside`. `user_agent` is like
    /// `portside-desktop/0.1.0`.
    pub fn new(base_url: &str, user_agent: &str) -> Result<Client, ApiError> {
        let mut base = Url::parse(base_url).map_err(|e| ApiError::BaseUrl(e.to_string()))?;
        if !matches!(base.scheme(), "http" | "https") {
            return Err(ApiError::BaseUrl(format!(
                "{base_url} is not an http(s) URL"
            )));
        }
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        let http = reqwest::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(ApiError::Network)?;
        Ok(Client { base, http })
    }

    pub fn base_url(&self) -> &str {
        self.base.as_str()
    }

    /// `v1/ports/{id}.json`
    pub async fn port(&self, id: &str) -> Result<PortDetail, ApiError> {
        self.json(
            self.http
                .get(self.url(&["v1", "ports", &format!("{id}.json")])?),
        )
        .await
    }

    /// `v1/ports/{id}/releases.json`, newest first, revoked ones included.
    pub async fn releases(&self, id: &str) -> Result<Vec<Release>, ApiError> {
        self.json(
            self.http
                .get(self.url(&["v1", "ports", id, "releases.json"])?),
        )
        .await
    }

    /// `v1/games/{game_key}.json`
    pub async fn game(&self, game_key: &str) -> Result<Game, ApiError> {
        self.json(
            self.http
                .get(self.url(&["v1", "games", &format!("{game_key}.json")])?),
        )
        .await
    }

    /// `v1/snapshot.json`, every port. Pass the last `etag` to get `NotModified` when the
    /// file hasn't changed.
    pub async fn snapshot(&self, etag: Option<&str>) -> Result<SnapshotResult, ApiError> {
        let mut req = self.http.get(self.url(&["v1", "snapshot.json"])?);
        if let Some(etag) = etag {
            req = req.header(header::IF_NONE_MATCH, etag);
        }
        let res = req.send().await.map_err(ApiError::Network)?;
        let etag = res
            .headers()
            .get(header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        if res.status() == StatusCode::NOT_MODIFIED {
            return Ok(SnapshotResult::NotModified { etag });
        }
        let snapshot = Self::decode(res).await?;
        Ok(SnapshotResult::Snapshot { etag, snapshot })
    }

    /// Builds a URL from path segments; each segment is percent-encoded.
    fn url(&self, segments: &[&str]) -> Result<Url, ApiError> {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .map_err(|()| ApiError::BaseUrl(self.base.to_string()))?
            .pop_if_empty()
            .extend(segments);
        Ok(url)
    }

    async fn json<T: DeserializeOwned>(&self, req: reqwest::RequestBuilder) -> Result<T, ApiError> {
        let res = req.send().await.map_err(ApiError::Network)?;
        Self::decode(res).await
    }

    async fn decode<T: DeserializeOwned>(res: reqwest::Response) -> Result<T, ApiError> {
        let status = res.status();
        let request_id = res
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = res.bytes().await.map_err(ApiError::Network)?;
        if !status.is_success() {
            return Err(ApiError::Status {
                status: status.as_u16(),
                problem: serde_json::from_slice::<Problem>(&body).ok(),
                request_id,
            });
        }
        serde_json::from_slice(&body).map_err(|e| ApiError::Decode(e.to_string()))
    }
}
