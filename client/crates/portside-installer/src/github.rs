//! Reads release lists from the GitHub REST API, for ports the Portside API has no
//! releases for yet (client/docs/install-launch.md, step 3).

use portside_api::models::{ManifestSource, ReleaseChannel};
use regex::Regex;
use reqwest::StatusCode;
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue};
use serde::Deserialize;
use url::Url;

use crate::error::{InstallError, Result};
use crate::resolve::{ReleaseFile, UpstreamRelease};

pub const GITHUB_API: &str = "https://api.github.com";

#[derive(Debug, Clone)]
pub struct Github {
    http: reqwest::Client,
    api: Url,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
    /// `sha256:<hex>`, on files uploaded since mid-2025.
    #[serde(default)]
    digest: Option<String>,
}

impl From<GhRelease> for UpstreamRelease {
    fn from(r: GhRelease) -> Self {
        UpstreamRelease {
            tag: r.tag_name,
            html_url: r.html_url,
            files: r
                .assets
                .into_iter()
                .map(|a| ReleaseFile {
                    sha256: a
                        .digest
                        .as_deref()
                        .and_then(|d| d.strip_prefix("sha256:"))
                        .filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
                        .map(str::to_ascii_lowercase),
                    name: a.name,
                    url: a.browser_download_url,
                    size: a.size,
                })
                .collect(),
        }
    }
}

impl Github {
    /// `api` is `https://api.github.com` outside tests.
    pub fn new(api: &str, user_agent: &str) -> Result<Github> {
        let api = Url::parse(api).map_err(|e| InstallError::GithubStatus {
            status: 0,
            what: format!("API address {api}: {e}"),
        })?;
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static("2022-11-28"),
        );
        let http = reqwest::Client::builder()
            .user_agent(user_agent)
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(20))
            .build()?;
        Ok(Github { http, api })
    }

    /// The newest release the manifest's `source` selects.
    pub async fn latest(&self, source: &ManifestSource) -> Result<UpstreamRelease> {
        let repo = source.repo.as_deref().unwrap_or_default();
        if repo.split('/').count() != 2 || repo.split('/').any(str::is_empty) {
            return Err(InstallError::UnsupportedSource(format!("repo \"{repo}\"")));
        }
        let channel = source.channel.unwrap_or(ReleaseChannel::Stable);
        if channel == ReleaseChannel::Stable {
            let found: Option<GhRelease> = self
                .get(&format!("repos/{repo}/releases/latest"), repo)
                .await?;
            return found
                .map(Into::into)
                .ok_or_else(|| InstallError::NoRelease {
                    repo: repo.to_owned(),
                });
        }
        let pattern = match (channel, source.tag_pattern.clone().flatten()) {
            (ReleaseChannel::TagPattern, Some(p)) => {
                Some(Regex::new(&p).map_err(|e| InstallError::BadPattern {
                    pattern: p,
                    reason: e.to_string(),
                })?)
            }
            (ReleaseChannel::TagPattern, None) => {
                return Err(InstallError::UnsupportedSource(
                    "tag_pattern without a pattern".into(),
                ));
            }
            _ => None,
        };
        let list: Vec<GhRelease> = self
            .get(&format!("repos/{repo}/releases?per_page=30"), repo)
            .await?
            .unwrap_or_default();
        list.into_iter()
            .filter(|r| !r.draft)
            .find(|r| match &pattern {
                Some(re) => re.is_match(&r.tag_name),
                // `prerelease`: the newest release, prerelease or not.
                None => true,
            })
            .map(Into::into)
            .ok_or_else(|| InstallError::NoRelease {
                repo: repo.to_owned(),
            })
    }

    /// GET a path under the API; `None` on 404.
    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str, repo: &str) -> Result<Option<T>> {
        let url = self
            .api
            .join(path)
            .map_err(|e| InstallError::GithubStatus {
                status: 0,
                what: format!("{path}: {e}"),
            })?;
        let res = self.http.get(url).send().await?;
        let status = res.status();
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let limited = res
            .headers()
            .get("x-ratelimit-remaining")
            .is_some_and(|v| v.as_bytes() == b"0");
        if (status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS) && limited {
            let reset_at = res
                .headers()
                .get("x-ratelimit-reset")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            return Err(InstallError::RateLimited { reset_at });
        }
        if !status.is_success() {
            return Err(InstallError::GithubStatus {
                status: status.as_u16(),
                what: format!("releases of {repo}"),
            });
        }
        Ok(Some(res.json().await?))
    }
}
