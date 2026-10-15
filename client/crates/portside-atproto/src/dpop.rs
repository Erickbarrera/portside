//! DPoP proofs (RFC 9449): every token request and every authenticated call carries a JWT
//! signed with a key that only this device holds, so a leaked token is useless elsewhere.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use serde_json::{Value, json};
use url::Url;

use crate::error::{AtprotoError, Result};
use crate::util::{b64, b64_decode, now, origin, random_bytes, random_token, sha256_b64};

/// An ES256 key pair bound to one sign-in.
#[derive(Clone)]
pub struct DpopKey(SigningKey);

impl std::fmt::Debug for DpopKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DpopKey(..)")
    }
}

impl DpopKey {
    pub fn generate() -> Result<DpopKey> {
        loop {
            // Nearly every 32-byte string is a valid P-256 scalar; retry the rare one that isn't.
            if let Ok(key) = SigningKey::from_slice(&random_bytes::<32>()?) {
                return Ok(DpopKey(key));
            }
        }
    }

    /// Restores a key saved with [`DpopKey::secret`].
    pub fn from_secret(secret: &str) -> Result<DpopKey> {
        SigningKey::from_slice(&b64_decode(secret)?)
            .map(DpopKey)
            .map_err(|_| AtprotoError::Decode("stored sign-in key is damaged".to_owned()))
    }

    /// The private key, for the keychain only.
    pub fn secret(&self) -> String {
        b64(self.0.to_bytes())
    }

    pub fn public_jwk(&self) -> Value {
        let point = self.0.verifying_key().to_encoded_point(false);
        json!({
            "kty": "EC",
            "crv": "P-256",
            "x": point.x().map(b64).unwrap_or_default(),
            "y": point.y().map(b64).unwrap_or_default(),
        })
    }

    /// A proof for one request. `htu` loses its query and fragment, as RFC 9449 requires;
    /// `access_token` adds the `ath` hash for calls to the account host.
    pub fn proof(
        &self,
        htm: &str,
        htu: &Url,
        nonce: Option<&str>,
        access_token: Option<&str>,
    ) -> Result<String> {
        let mut target = htu.clone();
        target.set_query(None);
        target.set_fragment(None);
        let header = json!({ "typ": "dpop+jwt", "alg": "ES256", "jwk": self.public_jwk() });
        let mut claims = json!({
            "jti": random_token::<16>()?,
            "htm": htm,
            "htu": target.as_str(),
            "iat": now(),
        });
        if let Some(nonce) = nonce {
            claims["nonce"] = Value::from(nonce);
        }
        if let Some(token) = access_token {
            claims["ath"] = Value::from(sha256_b64(token));
        }
        let input = format!("{}.{}", b64(header.to_string()), b64(claims.to_string()));
        let signature: Signature = self.0.sign(input.as_bytes());
        Ok(format!("{input}.{}", b64(signature.to_bytes())))
    }
}

/// The latest `DPoP-Nonce` each server sent, by origin.
#[derive(Debug, Default)]
pub struct Nonces(Mutex<HashMap<String, String>>);

impl Nonces {
    pub fn get(&self, url: &Url) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&origin(url))
            .cloned()
    }

    pub fn remember(&self, url: &Url, headers: &reqwest::header::HeaderMap) {
        if let Some(nonce) = headers.get("dpop-nonce").and_then(|v| v.to_str().ok()) {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(origin(url), nonce.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::VerifyingKey;
    use p256::ecdsa::signature::Verifier;

    #[test]
    fn proof_is_a_verifiable_es256_jwt() {
        let key = DpopKey::generate().expect("key");
        let url = Url::parse("https://pds.example/xrpc/a.b.c?x=1#y").expect("url");
        let jwt = key
            .proof("GET", &url, Some("n1"), Some("tok"))
            .expect("proof");
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
        let header: Value =
            serde_json::from_slice(&b64_decode(parts[0]).expect("b64")).expect("json");
        let claims: Value =
            serde_json::from_slice(&b64_decode(parts[1]).expect("b64")).expect("json");
        assert_eq!(header["typ"], "dpop+jwt");
        assert_eq!(header["jwk"]["crv"], "P-256");
        assert_eq!(claims["htu"], "https://pds.example/xrpc/a.b.c");
        assert_eq!(claims["nonce"], "n1");
        assert_eq!(claims["ath"], sha256_b64("tok"));

        let point = p256::EncodedPoint::from_affine_coordinates(
            b64_decode(header["jwk"]["x"].as_str().expect("x"))
                .expect("x")
                .as_slice()
                .into(),
            b64_decode(header["jwk"]["y"].as_str().expect("y"))
                .expect("y")
                .as_slice()
                .into(),
            false,
        );
        let verifying = VerifyingKey::from_encoded_point(&point).expect("point");
        let sig = Signature::from_slice(&b64_decode(parts[2]).expect("sig")).expect("sig");
        verifying
            .verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &sig)
            .expect("valid signature");
    }

    #[test]
    fn secret_round_trips() {
        let key = DpopKey::generate().expect("key");
        let again = DpopKey::from_secret(&key.secret()).expect("restore");
        assert_eq!(key.public_jwk(), again.public_jwk());
    }
}
