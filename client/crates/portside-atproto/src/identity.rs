//! Handle → DID → account host (PDS), the first step of signing in and of checking who
//! a sign-in server says signed in.

use serde::Deserialize;
use url::Url;

use crate::error::{AtprotoError, Result};

/// Where DID PLC documents are published.
pub const PLC_DIRECTORY: &str = "https://plc.directory";
/// Resolves handles whose own domain doesn't answer over HTTPS (DNS TXT handles): Bluesky's
/// public index answers `com.atproto.identity.resolveHandle` for any handle.
pub const PUBLIC_APPVIEW: &str = "https://public.api.bsky.app";

/// An account: its DID, its verified handle (None if the handle doesn't point back) and the
/// host that holds its data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub did: String,
    pub handle: Option<String>,
    pub pds: Url,
}

#[derive(Debug, Clone)]
pub struct Resolver {
    http: reqwest::Client,
    plc: Url,
    appview: Url,
    /// Try `https://<handle>/.well-known/atproto-did` first. Off in tests.
    handle_https: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DidDocument {
    id: String,
    #[serde(default)]
    also_known_as: Vec<String>,
    #[serde(default)]
    service: Vec<Service>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Service {
    id: String,
    #[serde(rename = "type", default)]
    kind: String,
    service_endpoint: serde_json::Value,
}

#[derive(Deserialize)]
struct ResolvedHandle {
    did: String,
}

impl Resolver {
    pub fn new(http: reqwest::Client) -> Result<Resolver> {
        Resolver::with_endpoints(http, PLC_DIRECTORY, PUBLIC_APPVIEW, true)
    }

    pub fn with_endpoints(
        http: reqwest::Client,
        plc: &str,
        appview: &str,
        handle_https: bool,
    ) -> Result<Resolver> {
        let parse = |s: &str| {
            Url::parse(s).map_err(|e| AtprotoError::Resolve(format!("bad address {s}: {e}")))
        };
        Ok(Resolver {
            http,
            plc: parse(plc)?,
            appview: parse(appview)?,
            handle_https,
        })
    }

    /// Accepts `@alice.bsky.social`, `alice.bsky.social` or a DID.
    pub async fn resolve(&self, input: &str) -> Result<Identity> {
        let input = input.trim().trim_start_matches('@');
        if input.starts_with("did:") {
            return self.resolve_did(input).await;
        }
        let handle = normalize_handle(input)?;
        let did = self.resolve_handle(&handle).await?;
        let mut identity = self.resolve_did(&did).await?;
        // A DID document may name a different handle; trust the one that points both ways.
        if identity.handle.as_deref() != Some(handle.as_str()) {
            identity.handle = None;
        }
        Ok(identity)
    }

    pub async fn resolve_handle(&self, handle: &str) -> Result<String> {
        if self.handle_https
            && let Some(did) = self.well_known_did(handle).await
        {
            return Ok(did);
        }
        let mut url = self
            .appview
            .join("xrpc/com.atproto.identity.resolveHandle")
            .map_err(|e| AtprotoError::Resolve(e.to_string()))?;
        url.query_pairs_mut().append_pair("handle", handle);
        let resp = self
            .http
            .get(url.clone())
            .send()
            .await
            .map_err(|e| AtprotoError::network(&url, e))?;
        if resp.status().as_u16() == 400 || resp.status().as_u16() == 404 {
            return Err(AtprotoError::Resolve(format!(
                "No account found for {handle}. Check the spelling."
            )));
        }
        if !resp.status().is_success() {
            return Err(AtprotoError::Resolve(format!(
                "Couldn't look up {handle} right now ({}).",
                resp.status()
            )));
        }
        let body: ResolvedHandle = resp
            .json()
            .await
            .map_err(|e| AtprotoError::Decode(e.to_string()))?;
        check_did(&body.did)?;
        Ok(body.did)
    }

    async fn well_known_did(&self, handle: &str) -> Option<String> {
        let url = Url::parse(&format!("https://{handle}/.well-known/atproto-did")).ok()?;
        let resp = self.http.get(url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let text = resp.text().await.ok()?;
        let did = text.trim();
        (did.starts_with("did:") && check_did(did).is_ok()).then(|| did.to_owned())
    }

    pub async fn resolve_did(&self, did: &str) -> Result<Identity> {
        check_did(did)?;
        let url = if did.starts_with("did:plc:") {
            // Not `join`: "did:plc:…" would parse as an absolute URL with scheme "did".
            Url::parse(&format!(
                "{}/{did}",
                self.plc.as_str().trim_end_matches('/')
            ))
            .map_err(|e| AtprotoError::Resolve(e.to_string()))?
        } else if let Some(host) = did.strip_prefix("did:web:") {
            if host.contains(':') {
                return Err(AtprotoError::Resolve(format!(
                    "{did} uses a did:web with a path or port, which isn't supported."
                )));
            }
            Url::parse(&format!("https://{host}/.well-known/did.json"))
                .map_err(|e| AtprotoError::Resolve(e.to_string()))?
        } else {
            return Err(AtprotoError::Resolve(format!(
                "{did} uses a kind of DID that AT Protocol doesn't support."
            )));
        };
        let resp = self
            .http
            .get(url.clone())
            .send()
            .await
            .map_err(|e| AtprotoError::network(&url, e))?;
        if !resp.status().is_success() {
            return Err(AtprotoError::Resolve(format!(
                "Couldn't find the account {did} ({}).",
                resp.status()
            )));
        }
        let doc: DidDocument = resp
            .json()
            .await
            .map_err(|e| AtprotoError::Decode(format!("DID document: {e}")))?;
        identity_from(doc, did)
    }
}

fn identity_from(doc: DidDocument, did: &str) -> Result<Identity> {
    if doc.id != did {
        return Err(AtprotoError::Resolve(format!(
            "The DID document for {did} belongs to {}.",
            doc.id
        )));
    }
    let pds = doc
        .service
        .iter()
        .find(|s| s.id.ends_with("#atproto_pds") && s.kind == "AtprotoPersonalDataServer")
        .and_then(|s| s.service_endpoint.as_str())
        .and_then(|s| Url::parse(s).ok())
        .filter(|u| matches!(u.scheme(), "https" | "http"))
        .ok_or_else(|| {
            AtprotoError::Resolve(format!("The account {did} doesn't name an account host."))
        })?;
    let handle = doc
        .also_known_as
        .iter()
        .find_map(|a| a.strip_prefix("at://"))
        .map(str::to_ascii_lowercase);
    Ok(Identity {
        did: did.to_owned(),
        handle,
        pds,
    })
}

fn check_did(did: &str) -> Result<()> {
    let ok = did.len() <= 2048
        && did.starts_with("did:")
        && did.splitn(3, ':').count() == 3
        && did
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._:%-".contains(c));
    if ok {
        Ok(())
    } else {
        Err(AtprotoError::Resolve(format!("{did} isn't a valid DID.")))
    }
}

/// Lower-cases and checks a handle (atproto handle syntax: a domain name).
pub fn normalize_handle(input: &str) -> Result<String> {
    let handle = input.trim().trim_start_matches('@').to_ascii_lowercase();
    let labels: Vec<&str> = handle.split('.').collect();
    let ok = handle.len() <= 253
        && labels.len() >= 2
        && labels.iter().all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
        && labels
            .last()
            .is_some_and(|tld| tld.chars().next().is_some_and(|c| c.is_ascii_alphabetic()));
    if ok {
        Ok(handle)
    } else {
        Err(AtprotoError::Resolve(format!(
            "\"{}\" isn't a handle. It looks like name.bsky.social.",
            input.trim()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles() {
        assert_eq!(
            normalize_handle("@Alice.bsky.social").expect("ok"),
            "alice.bsky.social"
        );
        assert!(normalize_handle("alice").is_err());
        assert!(normalize_handle("alice..social").is_err());
        assert!(normalize_handle("alice.123").is_err());
    }

    #[test]
    fn did_documents() {
        let doc: DidDocument = serde_json::from_value(serde_json::json!({
            "id": "did:plc:abc",
            "alsoKnownAs": ["at://alice.example.com"],
            "service": [{
                "id": "#atproto_pds",
                "type": "AtprotoPersonalDataServer",
                "serviceEndpoint": "https://pds.example.com"
            }]
        }))
        .expect("doc");
        let id = identity_from(doc, "did:plc:abc").expect("identity");
        assert_eq!(id.handle.as_deref(), Some("alice.example.com"));
        assert_eq!(id.pds.as_str(), "https://pds.example.com/");
    }
}
