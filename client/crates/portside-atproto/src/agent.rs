//! Authenticated XRPC calls to the signed-in account's host, renewing the sign-in when it
//! runs out. Calls for other services go through the host with `atproto-proxy`.

use std::sync::Arc;

use reqwest::{Method, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use url::Url;

use crate::dpop::{DpopKey, Nonces};
use crate::error::{AtprotoError, Result};
use crate::oauth;
use crate::session::{Session, SessionStore};
use crate::util::now;

/// Renew this many seconds before the access token runs out.
const REFRESH_MARGIN: u64 = 60;

/// Bluesky's direct message service, reached through the account host.
pub const BSKY_CHAT_PROXY: &str = "did:web:api.bsky.chat#bsky_chat";

pub struct Agent {
    http: reqwest::Client,
    did: String,
    pds: Url,
    key: DpopKey,
    session: tokio::sync::Mutex<Session>,
    nonces: Nonces,
    store: Arc<dyn SessionStore>,
}

impl std::fmt::Debug for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Agent")
            .field("did", &self.did)
            .finish_non_exhaustive()
    }
}

#[derive(serde::Deserialize)]
struct XrpcErrorBody {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

impl Agent {
    /// `store` receives the session again every time it is renewed.
    pub fn new(
        http: reqwest::Client,
        session: Session,
        store: Arc<dyn SessionStore>,
    ) -> Result<Agent> {
        let mut pds = Url::parse(&session.pds)
            .map_err(|e| AtprotoError::Decode(format!("stored account host: {e}")))?;
        if !pds.path().ends_with('/') {
            let path = format!("{}/", pds.path());
            pds.set_path(&path);
        }
        Ok(Agent {
            http,
            did: session.did.clone(),
            pds,
            key: DpopKey::from_secret(&session.dpop_key)?,
            session: tokio::sync::Mutex::new(session),
            nonces: Nonces::default(),
            store,
        })
    }

    pub fn did(&self) -> &str {
        &self.did
    }

    pub async fn session(&self) -> Session {
        self.session.lock().await.clone()
    }

    /// `GET /xrpc/{nsid}`. Repeat a key for array parameters.
    pub async fn get<T: DeserializeOwned>(
        &self,
        nsid: &str,
        query: &[(&str, String)],
        proxy: Option<&str>,
    ) -> Result<T> {
        self.call(Method::GET, nsid, query, None, proxy).await
    }

    /// `POST /xrpc/{nsid}` with a JSON body.
    pub async fn post<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        nsid: &str,
        body: &B,
        proxy: Option<&str>,
    ) -> Result<T> {
        let body = serde_json::to_value(body).map_err(|e| AtprotoError::Decode(e.to_string()))?;
        self.call(Method::POST, nsid, &[], Some(body), proxy).await
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        nsid: &str,
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
        proxy: Option<&str>,
    ) -> Result<T> {
        let url = self
            .pds
            .join(&format!("xrpc/{nsid}"))
            .map_err(|e| AtprotoError::Decode(e.to_string()))?;
        let mut token = self.fresh_token().await?;
        let mut nonce_retried = false;
        let mut refreshed = false;
        loop {
            let proof = self.key.proof(
                method.as_str(),
                &url,
                self.nonces.get(&url).as_deref(),
                Some(&token),
            )?;
            let mut req = self
                .http
                .request(method.clone(), url.clone())
                .header("Authorization", format!("DPoP {token}"))
                .header("DPoP", proof);
            if !query.is_empty() {
                req = req.query(query);
            }
            if let Some(proxy) = proxy {
                req = req.header("atproto-proxy", proxy);
            }
            if let Some(body) = &body {
                req = req.json(body);
            }
            let resp = req
                .send()
                .await
                .map_err(|e| AtprotoError::network(&url, e))?;
            self.nonces.remember(&url, resp.headers());
            let status = resp.status();
            let challenge = resp
                .headers()
                .get("www-authenticate")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_owned();
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| AtprotoError::network(&url, e))?;
            if status.is_success() {
                let bytes: &[u8] = if bytes.is_empty() { b"{}" } else { &bytes };
                return serde_json::from_slice(bytes)
                    .map_err(|e| AtprotoError::Decode(format!("{nsid}: {e}")));
            }
            let err = serde_json::from_slice::<XrpcErrorBody>(&bytes).ok();
            let err_name = err.as_ref().and_then(|e| e.error.clone());
            if status == StatusCode::UNAUTHORIZED || status == StatusCode::BAD_REQUEST {
                if !nonce_retried
                    && (challenge.contains("use_dpop_nonce")
                        || err_name.as_deref() == Some("use_dpop_nonce"))
                {
                    nonce_retried = true;
                    continue;
                }
                if status == StatusCode::UNAUTHORIZED
                    && !refreshed
                    && (challenge.contains("invalid_token")
                        || matches!(
                            err_name.as_deref(),
                            Some("invalid_token" | "ExpiredToken" | "InvalidToken")
                        ))
                {
                    refreshed = true;
                    token = self.refresh_after_reject(&token).await?;
                    continue;
                }
            }
            return Err(AtprotoError::Xrpc {
                nsid: nsid.to_owned(),
                status: status.as_u16(),
                error: err_name,
                message: err.and_then(|e| e.message),
            });
        }
    }

    /// The current access token, renewed first if it's about to run out.
    async fn fresh_token(&self) -> Result<String> {
        let mut session = self.session.lock().await;
        if session.expires_at <= now() + REFRESH_MARGIN {
            self.renew(&mut session).await?;
        }
        Ok(session.access_token.clone())
    }

    /// The host rejected `used`; renew unless another call already did.
    async fn refresh_after_reject(&self, used: &str) -> Result<String> {
        let mut session = self.session.lock().await;
        if session.access_token == used {
            self.renew(&mut session).await?;
        }
        Ok(session.access_token.clone())
    }

    async fn renew(&self, session: &mut Session) -> Result<()> {
        let renewed = oauth::refresh(&self.http, &self.nonces, session).await?;
        self.store.save(&renewed)?;
        *session = renewed;
        Ok(())
    }
}
