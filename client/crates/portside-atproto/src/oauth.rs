//! AT Protocol OAuth for a public native client: PAR + PKCE + DPoP, the browser for
//! consent and a loopback redirect back to the app (atproto OAuth spec, "native clients").

use std::sync::Arc;

use serde::Deserialize;
use url::Url;

use crate::dpop::{DpopKey, Nonces};
use crate::error::{AtprotoError, Result};
use crate::identity::{Identity, Resolver};
use crate::session::Session;
use crate::util::{now, random_token, sha256_b64};

/// What Portside asks for: the account (`atproto`), posting and following
/// (`transition:generic`) and Bluesky direct messages (`transition:chat.bsky`).
/// [TBD: move to granular permission sets once Bluesky's app shows them well.]
pub const DEFAULT_SCOPE: &str = "atproto transition:generic transition:chat.bsky";

/// Where "Sign in with Bluesky" starts when no handle is given.
pub const BLUESKY_SIGN_IN: &str = "https://bsky.social";

/// Who Portside is to the sign-in server.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub client_id: String,
    /// Registered redirect without a port; the listener's port is added per sign-in.
    pub redirect_uri: Url,
    pub scope: String,
}

impl ClientConfig {
    /// AT Protocol's "localhost client": no hosted metadata document; the client id itself
    /// carries the redirect address and scope. Accepted by Bluesky's sign-in server.
    pub fn loopback(scope: &str) -> Result<ClientConfig> {
        let redirect = "http://127.0.0.1/callback";
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("redirect_uri", redirect)
            .append_pair("scope", scope)
            .finish();
        Ok(ClientConfig {
            client_id: format!("http://localhost?{query}"),
            redirect_uri: Url::parse(redirect).map_err(|e| AtprotoError::Decode(e.to_string()))?,
            scope: scope.to_owned(),
        })
    }
}

/// Authorization server metadata (RFC 8414), the fields Portside uses.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub pushed_authorization_request_endpoint: Option<String>,
    #[serde(default)]
    pub revocation_endpoint: Option<String>,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
}

#[derive(Deserialize)]
struct ProtectedResource {
    #[serde(default)]
    authorization_servers: Vec<String>,
}

#[derive(Deserialize)]
struct ParResponse {
    request_uri: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    sub: Option<String>,
}

#[derive(Deserialize)]
struct OAuthErrorBody {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

/// The query the browser brings back to the redirect address.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub iss: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

impl CallbackParams {
    pub fn from_url(url: &Url) -> CallbackParams {
        let mut p = CallbackParams::default();
        for (k, v) in url.query_pairs() {
            let v = Some(v.into_owned());
            match k.as_ref() {
                "code" => p.code = v,
                "state" => p.state = v,
                "iss" => p.iss = v,
                "error" => p.error = v,
                "error_description" => p.error_description = v,
                _ => {}
            }
        }
        p
    }
}

/// A sign-in waiting for the browser.
#[derive(Debug)]
pub struct PendingSignIn {
    /// Open this in the system browser.
    pub authorize_url: Url,
    state: String,
    verifier: String,
    key: DpopKey,
    server: ServerMetadata,
    redirect_uri: Url,
    expected: Option<Identity>,
}

#[derive(Debug, Clone)]
pub struct OAuthClient {
    http: reqwest::Client,
    resolver: Resolver,
    config: ClientConfig,
    nonces: Arc<Nonces>,
}

impl PendingSignIn {
    /// The `state` sent with this sign-in; the browser must bring it back.
    pub fn state(&self) -> &str {
        &self.state
    }
}

impl OAuthClient {
    pub fn new(http: reqwest::Client, resolver: Resolver, config: ClientConfig) -> OAuthClient {
        OAuthClient {
            http,
            resolver,
            config,
            nonces: Arc::new(Nonces::default()),
        }
    }

    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    pub fn resolver(&self) -> &Resolver {
        &self.resolver
    }

    /// Starts signing in. `account` is a handle or DID, an account host address
    /// (`https://…`), or empty for Bluesky. `redirect_uri` is the listener's address.
    pub async fn begin(&self, account: &str, redirect_uri: &Url) -> Result<PendingSignIn> {
        let account = account.trim();
        let (expected, host) = if account.is_empty() {
            (None, parse_url(BLUESKY_SIGN_IN)?)
        } else if account.starts_with("https://") || account.starts_with("http://") {
            (None, parse_url(account)?)
        } else {
            let identity = self.resolver.resolve(account).await?;
            let pds = identity.pds.clone();
            (Some(identity), pds)
        };
        let issuer = match self.authorization_server(&host).await {
            Ok(issuer) => issuer,
            // An entryway such as bsky.social is its own sign-in server.
            Err(_) if expected.is_none() => host.as_str().trim_end_matches('/').to_owned(),
            Err(e) => return Err(e),
        };
        let server = self.server_metadata(&issuer).await?;
        let par_endpoint = server
            .pushed_authorization_request_endpoint
            .clone()
            .ok_or_else(|| {
                AtprotoError::Resolve(format!(
                    "{issuer} doesn't support the sign-in method Portside uses."
                ))
            })?;
        if !server.scopes_supported.is_empty()
            && !server.scopes_supported.iter().any(|s| s == "atproto")
        {
            return Err(AtprotoError::Resolve(format!(
                "{issuer} isn't an AT Protocol sign-in server."
            )));
        }

        let key = DpopKey::generate()?;
        let state = random_token::<24>()?;
        let verifier = random_token::<32>()?;
        let challenge = sha256_b64(&verifier);
        let login_hint = expected
            .as_ref()
            .map(|i| i.handle.clone().unwrap_or_else(|| i.did.clone()));
        let mut form = vec![
            ("client_id", self.config.client_id.clone()),
            ("response_type", "code".to_owned()),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256".to_owned()),
            ("state", state.clone()),
            ("redirect_uri", redirect_uri.to_string()),
            ("scope", self.config.scope.clone()),
        ];
        if let Some(hint) = login_hint {
            form.push(("login_hint", hint));
        }
        let par: ParResponse = self
            .dpop_form(&key, &parse_url(&par_endpoint)?, &form)
            .await?;

        let mut authorize_url = parse_url(&server.authorization_endpoint)?;
        authorize_url
            .query_pairs_mut()
            .append_pair("client_id", &self.config.client_id)
            .append_pair("request_uri", &par.request_uri);
        Ok(PendingSignIn {
            authorize_url,
            state,
            verifier,
            key,
            server,
            redirect_uri: redirect_uri.clone(),
            expected,
        })
    }

    /// Finishes signing in with what the browser brought back.
    pub async fn complete(
        &self,
        pending: PendingSignIn,
        params: CallbackParams,
    ) -> Result<Session> {
        if params.state.as_deref() != Some(pending.state.as_str()) {
            return Err(AtprotoError::OAuth {
                error: "state_mismatch".to_owned(),
                description: Some("the browser returned from a different sign-in".to_owned()),
            });
        }
        if let Some(error) = params.error {
            return Err(if error == "access_denied" {
                AtprotoError::Denied
            } else {
                AtprotoError::OAuth {
                    error,
                    description: params.error_description,
                }
            });
        }
        if let Some(iss) = &params.iss
            && !same_url(iss, &pending.server.issuer)
        {
            return Err(AtprotoError::OAuth {
                error: "issuer_mismatch".to_owned(),
                description: Some(format!("expected {}, got {iss}", pending.server.issuer)),
            });
        }
        let code = params
            .code
            .ok_or_else(|| AtprotoError::Decode("the browser returned no code".to_owned()))?;
        let token_endpoint = parse_url(&pending.server.token_endpoint)?;
        let tokens: TokenResponse = self
            .dpop_form(
                &pending.key,
                &token_endpoint,
                &[
                    ("grant_type", "authorization_code".to_owned()),
                    ("code", code),
                    ("redirect_uri", pending.redirect_uri.to_string()),
                    ("client_id", self.config.client_id.clone()),
                    ("code_verifier", pending.verifier.clone()),
                ],
            )
            .await?;
        let (sub, scope) = check_tokens(&tokens)?;

        // The sign-in server must be the one in charge of the account it names (spec,
        // "Identity Authentication").
        let identity = match pending.expected {
            Some(expected) if expected.did == sub => expected,
            Some(expected) => {
                return Err(AtprotoError::OAuth {
                    error: "account_mismatch".to_owned(),
                    description: Some(format!("signed in as {sub}, expected {}", expected.did)),
                });
            }
            None => {
                let identity = self.resolver.resolve_did(&sub).await?;
                let issuer = self.authorization_server(&identity.pds).await?;
                if !same_url(&issuer, &pending.server.issuer) {
                    return Err(AtprotoError::OAuth {
                        error: "issuer_mismatch".to_owned(),
                        description: Some(format!(
                            "{sub} isn't managed by {}",
                            pending.server.issuer
                        )),
                    });
                }
                identity
            }
        };

        Ok(Session {
            did: sub,
            handle: identity.handle,
            pds: identity.pds.to_string(),
            issuer: pending.server.issuer,
            token_endpoint: pending.server.token_endpoint,
            revocation_endpoint: pending.server.revocation_endpoint,
            client_id: self.config.client_id.clone(),
            scope,
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            expires_at: now() + tokens.expires_in.unwrap_or(300),
            dpop_key: pending.key.secret(),
        })
    }

    /// The sign-in server in charge of an account host
    /// (`/.well-known/oauth-protected-resource`).
    pub async fn authorization_server(&self, pds: &Url) -> Result<String> {
        let url = pds
            .join("/.well-known/oauth-protected-resource")
            .map_err(|e| AtprotoError::Resolve(e.to_string()))?;
        let resource: ProtectedResource = self.get_json(&url).await?;
        resource
            .authorization_servers
            .into_iter()
            .next()
            .map(|s| s.trim_end_matches('/').to_owned())
            .ok_or_else(|| {
                AtprotoError::Resolve(format!("{} doesn't name a sign-in server.", pds.as_str()))
            })
    }

    async fn server_metadata(&self, issuer: &str) -> Result<ServerMetadata> {
        let url = parse_url(issuer)?
            .join("/.well-known/oauth-authorization-server")
            .map_err(|e| AtprotoError::Resolve(e.to_string()))?;
        let meta: ServerMetadata = self.get_json(&url).await?;
        if !same_url(&meta.issuer, issuer) {
            return Err(AtprotoError::Resolve(format!(
                "{issuer} describes itself as {}.",
                meta.issuer
            )));
        }
        Ok(meta)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &Url) -> Result<T> {
        let resp = self
            .http
            .get(url.clone())
            .send()
            .await
            .map_err(|e| AtprotoError::network(url, e))?;
        if !resp.status().is_success() {
            return Err(AtprotoError::Resolve(format!(
                "{} answered {}.",
                url.host_str().unwrap_or("the server"),
                resp.status()
            )));
        }
        resp.json()
            .await
            .map_err(|e| AtprotoError::Decode(format!("{url}: {e}")))
    }

    async fn dpop_form<T: serde::de::DeserializeOwned>(
        &self,
        key: &DpopKey,
        url: &Url,
        form: &[(&str, String)],
    ) -> Result<T> {
        dpop_form(&self.http, &self.nonces, key, url, form).await
    }
}

/// Renews a sign-in with its refresh token.
pub(crate) async fn refresh(
    http: &reqwest::Client,
    nonces: &Nonces,
    session: &Session,
) -> Result<Session> {
    let refresh_token = session
        .refresh_token
        .clone()
        .ok_or(AtprotoError::SessionExpired)?;
    let key = DpopKey::from_secret(&session.dpop_key)?;
    let endpoint = parse_url(&session.token_endpoint)?;
    let tokens: TokenResponse = match dpop_form(
        http,
        nonces,
        &key,
        &endpoint,
        &[
            ("grant_type", "refresh_token".to_owned()),
            ("refresh_token", refresh_token),
            ("client_id", session.client_id.clone()),
        ],
    )
    .await
    {
        Err(AtprotoError::OAuth { error, .. }) if error == "invalid_grant" => {
            return Err(AtprotoError::SessionExpired);
        }
        other => other?,
    };
    let (sub, scope) = check_tokens(&tokens)?;
    if sub != session.did {
        return Err(AtprotoError::SessionExpired);
    }
    Ok(Session {
        scope,
        access_token: tokens.access_token,
        refresh_token: tokens
            .refresh_token
            .or_else(|| session.refresh_token.clone()),
        expires_at: now() + tokens.expires_in.unwrap_or(300),
        ..session.clone()
    })
}

/// Tells the sign-in server to forget the tokens. Best effort: signing out on the device
/// happens whatever this returns.
pub async fn revoke(http: &reqwest::Client, session: &Session) -> Result<()> {
    let Some(endpoint) = &session.revocation_endpoint else {
        return Ok(());
    };
    let key = DpopKey::from_secret(&session.dpop_key)?;
    let nonces = Nonces::default();
    let token = session
        .refresh_token
        .clone()
        .unwrap_or_else(|| session.access_token.clone());
    let _: serde::de::IgnoredAny = dpop_form(
        http,
        &nonces,
        &key,
        &parse_url(endpoint)?,
        &[("token", token), ("client_id", session.client_id.clone())],
    )
    .await?;
    Ok(())
}

fn check_tokens(tokens: &TokenResponse) -> Result<(String, String)> {
    if !tokens.token_type.eq_ignore_ascii_case("DPoP") {
        return Err(AtprotoError::Decode(format!(
            "token type {} instead of DPoP",
            tokens.token_type
        )));
    }
    let scope = tokens.scope.clone().unwrap_or_default();
    if !scope.split_whitespace().any(|s| s == "atproto") {
        return Err(AtprotoError::Decode(
            "the sign-in server didn't grant the atproto scope".to_owned(),
        ));
    }
    let sub = tokens
        .sub
        .clone()
        .filter(|s| s.starts_with("did:"))
        .ok_or_else(|| AtprotoError::Decode("the token names no account".to_owned()))?;
    Ok((sub, scope))
}

/// POSTs a form with a DPoP proof, retrying once with the server's nonce.
async fn dpop_form<T: serde::de::DeserializeOwned>(
    http: &reqwest::Client,
    nonces: &Nonces,
    key: &DpopKey,
    url: &Url,
    form: &[(&str, String)],
) -> Result<T> {
    let mut retried = false;
    loop {
        let proof = key.proof("POST", url, nonces.get(url).as_deref(), None)?;
        let resp = http
            .post(url.clone())
            .header("DPoP", proof)
            .form(form)
            .send()
            .await
            .map_err(|e| AtprotoError::network(url, e))?;
        nonces.remember(url, resp.headers());
        let status = resp.status();
        let body = resp
            .bytes()
            .await
            .map_err(|e| AtprotoError::network(url, e))?;
        if status.is_success() {
            let body: &[u8] = if body.is_empty() { b"{}" } else { &body };
            return serde_json::from_slice(body)
                .map_err(|e| AtprotoError::Decode(format!("{url}: {e}")));
        }
        let error = serde_json::from_slice::<OAuthErrorBody>(&body).ok();
        if !retried && error.as_ref().is_some_and(|e| e.error == "use_dpop_nonce") {
            retried = true;
            continue;
        }
        return Err(match error {
            Some(e) => AtprotoError::OAuth {
                error: e.error,
                description: e.error_description,
            },
            None => AtprotoError::OAuth {
                error: format!("http_{}", status.as_u16()),
                description: None,
            },
        });
    }
}

fn parse_url(s: &str) -> Result<Url> {
    Url::parse(s).map_err(|e| AtprotoError::Resolve(format!("bad address {s}: {e}")))
}

fn same_url(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_client_id_carries_redirect_and_scope() {
        let c = ClientConfig::loopback(DEFAULT_SCOPE).expect("config");
        let id = Url::parse(&c.client_id).expect("url");
        assert_eq!(id.host_str(), Some("localhost"));
        assert_eq!(id.port(), None);
        let q: Vec<(String, String)> = id.query_pairs().into_owned().collect();
        assert!(q.contains(&("redirect_uri".into(), "http://127.0.0.1/callback".into())));
        assert!(q.contains(&("scope".into(), DEFAULT_SCOPE.into())));
    }
}
