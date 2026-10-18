//! Resumable downloads, from allowed hosts only (client/CLAUDE.md, "Downloads").

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::StatusCode;
use reqwest::header::{CONTENT_RANGE, LOCATION, RANGE};
use tokio::io::AsyncWriteExt;
use url::Url;

use crate::error::{InstallError, IoContext, Result};

/// Hosts a download may start at or be redirected to.
#[derive(Debug, Clone)]
pub struct HostPolicy {
    hosts: Vec<String>,
    allow_http: bool,
}

impl HostPolicy {
    /// GitHub release downloads: `github.com` answers with a redirect to one of its file hosts.
    pub fn github() -> HostPolicy {
        HostPolicy {
            hosts: [
                "github.com",
                "objects.githubusercontent.com",
                "release-assets.githubusercontent.com",
            ]
            .map(str::to_owned)
            .to_vec(),
            allow_http: false,
        }
    }

    /// The same rules for other hosts: a `url_list` manifest's own (ADR-0024).
    pub fn with_hosts(&self, hosts: &[String]) -> HostPolicy {
        HostPolicy {
            hosts: hosts.to_vec(),
            allow_http: self.allow_http,
        }
    }

    /// Plain-HTTP local hosts, for tests against a mock server.
    pub fn local_for_tests(hosts: &[&str]) -> HostPolicy {
        HostPolicy {
            hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
            allow_http: true,
        }
    }

    pub fn allows(&self, url: &Url) -> bool {
        let scheme_ok = url.scheme() == "https" || (self.allow_http && url.scheme() == "http");
        scheme_ok
            && url
                .host_str()
                .is_some_and(|h| self.hosts.iter().any(|a| a.eq_ignore_ascii_case(h)))
    }

    pub fn check(&self, url: &str) -> Result<Url> {
        let parsed = Url::parse(url).map_err(|_| InstallError::HostNotAllowed(url.to_owned()))?;
        if self.allows(&parsed) {
            Ok(parsed)
        } else {
            Err(InstallError::HostNotAllowed(
                parsed.host_str().unwrap_or(url).to_owned(),
            ))
        }
    }
}

#[derive(Debug, Clone)]
pub struct Downloader {
    http: reqwest::Client,
    policy: HostPolicy,
}

impl Downloader {
    pub fn new(policy: HostPolicy, user_agent: &str) -> Result<Downloader> {
        let redirects = policy.clone();
        let http = portside_core::http::client_builder()
            .user_agent(user_agent)
            // Byte ranges must refer to the file as stored, not a decompressed copy.
            .no_gzip()
            .connect_timeout(Duration::from_secs(20))
            .read_timeout(Duration::from_secs(60))
            // A redirect to a host outside the policy is not followed; `fetch` reports it.
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= 10 {
                    attempt.error("too many redirects")
                } else if redirects.allows(attempt.url()) {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()?;
        Ok(Downloader { http, policy })
    }

    pub fn policy(&self) -> &HostPolicy {
        &self.policy
    }

    /// Downloads `url` into `part`, continuing a previous partial download when the server
    /// supports ranges. Calls `progress(done, total)` as bytes arrive. `expected_size`, when
    /// known, must match the finished file.
    pub async fn fetch(
        &self,
        url: &str,
        part: &Path,
        expected_size: Option<u64>,
        cancel: &Arc<AtomicBool>,
        progress: &(dyn Fn(u64, Option<u64>) + Send + Sync),
    ) -> Result<u64> {
        let url = self.policy.check(url)?;
        if let Some(dir) = part.parent() {
            tokio::fs::create_dir_all(dir).await.at(dir)?;
        }
        let mut have = tokio::fs::metadata(part)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        if expected_size.is_some_and(|size| have > size) {
            have = 0;
        }
        if have > 0 && Some(have) == expected_size {
            progress(have, expected_size);
            return Ok(have);
        }

        let mut req = self.http.get(url.clone());
        if have > 0 {
            req = req.header(RANGE, format!("bytes={have}-"));
        }
        let res = req.send().await?;
        let status = res.status();
        if status.is_redirection() {
            let to = res
                .headers()
                .get(LOCATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| res.url().join(v).ok());
            let host = to
                .as_ref()
                .and_then(|u| u.host_str())
                .unwrap_or("an unknown host");
            return Err(InstallError::HostNotAllowed(host.to_owned()));
        }
        let resumed = status == StatusCode::PARTIAL_CONTENT && range_starts_at(&res, have);
        let bad_range = status == StatusCode::PARTIAL_CONTENT && !resumed;
        if have > 0 && (status == StatusCode::RANGE_NOT_SATISFIABLE || bad_range) {
            // The partial file can't be continued; start over without a range.
            tokio::fs::remove_file(part).await.at(part)?;
            return Box::pin(self.fetch(url.as_str(), part, expected_size, cancel, progress)).await;
        }
        if !status.is_success() {
            return Err(InstallError::DownloadStatus(status.as_u16()));
        }
        let start = if resumed { have } else { 0 };
        let total = expected_size.or_else(|| res.content_length().map(|n| n + start));

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(resumed)
            .truncate(!resumed)
            .open(part)
            .await
            .at(part)?;
        let mut done = start;
        progress(done, total);
        let mut stream = res.bytes_stream();
        while let Some(chunk) = stream.next().await {
            if cancel.load(Ordering::Relaxed) {
                file.flush().await.at(part)?;
                return Err(InstallError::Cancelled);
            }
            let chunk = chunk?;
            file.write_all(&chunk).await.at(part)?;
            done += chunk.len() as u64;
            progress(done, total);
        }
        file.flush().await.at(part)?;
        file.sync_all().await.at(part)?;
        if let Some(expected) = expected_size
            && expected != done
        {
            // Keep nothing from a bad download, so the next try starts clean.
            tokio::fs::remove_file(part).await.at(part)?;
            return Err(InstallError::SizeMismatch {
                expected,
                actual: done,
            });
        }
        Ok(done)
    }
}

/// True when a 206 response continues at byte `from`.
fn range_starts_at(res: &reqwest::Response, from: u64) -> bool {
    res.headers()
        .get(CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes "))
        .and_then(|v| v.split('-').next())
        .and_then(|v| v.parse::<u64>().ok())
        == Some(from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_policy_is_https_and_github_only() {
        let p = HostPolicy::github();
        assert!(
            p.check("https://github.com/a/b/releases/download/v1/x.zip")
                .is_ok()
        );
        assert!(
            p.check("https://release-assets.githubusercontent.com/x")
                .is_ok()
        );
        assert!(p.check("http://github.com/a/b").is_err());
        assert!(p.check("https://example.com/x.zip").is_err());
        assert!(p.check("https://github.com.example.com/x.zip").is_err());
        assert!(p.check("not a url").is_err());
    }
}
