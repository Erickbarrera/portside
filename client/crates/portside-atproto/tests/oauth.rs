//! The whole sign-in and an authenticated call against a mock account host and sign-in
//! server (synthetic accounts only).

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use portside_atproto::oauth::DEFAULT_SCOPE;
use portside_atproto::{
    Agent, AtprotoError, CallbackParams, ClientConfig, MemoryStore, OAuthClient, Resolver,
    SessionStore,
};
use serde_json::{Value, json};
use url::Url;
use wiremock::matchers::{body_string_contains, header, method, path, query_param};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

/// Matches requests whose DPoP proof carries `nonce`.
struct ProofNonce(&'static str);

impl Match for ProofNonce {
    fn matches(&self, request: &Request) -> bool {
        claims(request).is_some_and(|c| c["nonce"] == self.0)
    }
}

fn claims(request: &Request) -> Option<Value> {
    let proof = request.headers.get("dpop")?.to_str().ok()?;
    let payload = proof.split('.').nth(1)?;
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()
}

async fn mount_account(server: &MockServer) {
    let base = server.uri();
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.identity.resolveHandle"))
        .and(query_param("handle", "alice.test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "did": "did:plc:alice" })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/did:plc:alice"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "did:plc:alice",
            "alsoKnownAs": ["at://alice.test"],
            "service": [{
                "id": "#atproto_pds",
                "type": "AtprotoPersonalDataServer",
                "serviceEndpoint": base
            }]
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-protected-resource"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "authorization_servers": [base] })),
        )
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": base,
            "authorization_endpoint": format!("{base}/oauth/authorize"),
            "token_endpoint": format!("{base}/oauth/token"),
            "pushed_authorization_request_endpoint": format!("{base}/oauth/par"),
            "scopes_supported": ["atproto", "transition:generic", "transition:chat.bsky"]
        })))
        .mount(server)
        .await;
}

fn client(server: &MockServer) -> OAuthClient {
    let http = reqwest::Client::new();
    let resolver = Resolver::with_endpoints(http.clone(), &server.uri(), &server.uri(), false)
        .expect("resolver");
    OAuthClient::new(
        http,
        resolver,
        ClientConfig::loopback(DEFAULT_SCOPE).expect("config"),
    )
}

fn token_body(access: &str) -> Value {
    json!({
        "access_token": access,
        "token_type": "DPoP",
        "expires_in": 3600,
        "refresh_token": format!("refresh-{access}"),
        "scope": DEFAULT_SCOPE,
        "sub": "did:plc:alice"
    })
}

#[tokio::test]
async fn signs_in_and_calls_the_account_host() {
    let server = MockServer::start().await;
    mount_account(&server).await;
    // The server wants a nonce first (RFC 9449 §8).
    Mock::given(method("POST"))
        .and(path("/oauth/par"))
        .respond_with(
            ResponseTemplate::new(400)
                .insert_header("DPoP-Nonce", "n-1")
                .set_body_json(json!({ "error": "use_dpop_nonce" })),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/par"))
        .and(ProofNonce("n-1"))
        .and(body_string_contains("code_challenge_method=S256"))
        .and(body_string_contains("login_hint=alice.test"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(json!({ "request_uri": "urn:req:1", "expires_in": 60 })),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(ProofNonce("n-1"))
        .and(body_string_contains("grant_type=authorization_code"))
        .and(body_string_contains("code=the-code"))
        .and(body_string_contains("code_verifier="))
        .respond_with(ResponseTemplate::new(200).set_body_json(token_body("at-1")))
        .expect(1)
        .mount(&server)
        .await;

    let oauth = client(&server);
    let redirect = Url::parse("http://127.0.0.1:4567/callback").expect("url");
    let pending = oauth.begin("@Alice.test", &redirect).await.expect("begin");
    assert!(
        pending
            .authorize_url
            .as_str()
            .starts_with(&format!("{}/oauth/authorize?", server.uri()))
    );
    assert!(
        pending
            .authorize_url
            .as_str()
            .contains("request_uri=urn%3Areq%3A1")
    );

    let params = CallbackParams {
        code: Some("the-code".into()),
        state: Some(pending.state().to_owned()),
        iss: Some(server.uri()),
        ..CallbackParams::default()
    };
    let session = oauth.complete(pending, params).await.expect("complete");
    assert_eq!(session.did, "did:plc:alice");
    assert_eq!(session.handle.as_deref(), Some("alice.test"));
    assert!(session.has_scope("transition:chat.bsky"));

    // The account host wants its own nonce too.
    Mock::given(method("GET"))
        .and(path("/xrpc/app.bsky.actor.getProfile"))
        .respond_with(
            ResponseTemplate::new(401)
                .insert_header("DPoP-Nonce", "pds-1")
                .insert_header("WWW-Authenticate", "DPoP error=\"use_dpop_nonce\""),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/xrpc/app.bsky.actor.getProfile"))
        .and(header("authorization", "DPoP at-1"))
        .and(ProofNonce("pds-1"))
        .and(query_param("actor", "did:plc:alice"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "did": "did:plc:alice" })))
        .expect(1)
        .mount(&server)
        .await;
    let store = Arc::new(MemoryStore::default());
    let agent = Agent::new(reqwest::Client::new(), session, store).expect("agent");
    let profile: Value = agent
        .get(
            "app.bsky.actor.getProfile",
            &[("actor", "did:plc:alice".to_owned())],
            None,
        )
        .await
        .expect("profile");
    assert_eq!(profile["did"], "did:plc:alice");
}

#[tokio::test]
async fn a_wrong_state_is_refused() {
    let server = MockServer::start().await;
    mount_account(&server).await;
    Mock::given(method("POST"))
        .and(path("/oauth/par"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(json!({ "request_uri": "urn:req:2" })),
        )
        .mount(&server)
        .await;
    let oauth = client(&server);
    let redirect = Url::parse("http://127.0.0.1:4567/callback").expect("url");
    let pending = oauth.begin("alice.test", &redirect).await.expect("begin");
    let err = oauth
        .complete(
            pending,
            CallbackParams {
                code: Some("c".into()),
                state: Some("someone-else".into()),
                ..CallbackParams::default()
            },
        )
        .await
        .expect_err("refused");
    assert!(matches!(err, AtprotoError::OAuth { ref error, .. } if error == "state_mismatch"));
}

#[tokio::test]
async fn saying_no_is_reported_as_denied() {
    let server = MockServer::start().await;
    mount_account(&server).await;
    Mock::given(method("POST"))
        .and(path("/oauth/par"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(json!({ "request_uri": "urn:req:3" })),
        )
        .mount(&server)
        .await;
    let oauth = client(&server);
    let redirect = Url::parse("http://127.0.0.1:4567/callback").expect("url");
    let pending = oauth.begin("alice.test", &redirect).await.expect("begin");
    let state = pending.state().to_owned();
    let err = oauth
        .complete(
            pending,
            CallbackParams {
                state: Some(state),
                error: Some("access_denied".into()),
                ..CallbackParams::default()
            },
        )
        .await
        .expect_err("denied");
    assert!(matches!(err, AtprotoError::Denied));
}

#[tokio::test]
async fn an_expired_token_is_renewed_and_saved() {
    let server = MockServer::start().await;
    mount_account(&server).await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=refresh-at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(token_body("at-2")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.repo.createRecord"))
        .and(header("authorization", "DPoP at-2"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "uri": "at://x", "cid": "c" })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let session = portside_atproto::Session {
        did: "did:plc:alice".into(),
        handle: Some("alice.test".into()),
        pds: server.uri(),
        issuer: server.uri(),
        token_endpoint: format!("{}/oauth/token", server.uri()),
        revocation_endpoint: None,
        client_id: "http://localhost".into(),
        scope: DEFAULT_SCOPE.into(),
        access_token: "at-1".into(),
        refresh_token: Some("refresh-at-1".into()),
        expires_at: 0,
        dpop_key: portside_atproto::DpopKey::generate().expect("key").secret(),
    };
    let store = Arc::new(MemoryStore::default());
    let agent = Agent::new(reqwest::Client::new(), session, store.clone()).expect("agent");
    let _: Value = agent
        .post(
            "com.atproto.repo.createRecord",
            &json!({ "repo": "did:plc:alice" }),
            None,
        )
        .await
        .expect("call");
    let saved = store.load().expect("load").expect("saved");
    assert_eq!(saved.access_token, "at-2");
}

#[tokio::test]
async fn a_dead_refresh_token_means_signing_in_again() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({ "error": "invalid_grant" })))
        .mount(&server)
        .await;
    let session = portside_atproto::Session {
        did: "did:plc:alice".into(),
        handle: None,
        pds: server.uri(),
        issuer: server.uri(),
        token_endpoint: format!("{}/oauth/token", server.uri()),
        revocation_endpoint: None,
        client_id: "http://localhost".into(),
        scope: DEFAULT_SCOPE.into(),
        access_token: "old".into(),
        refresh_token: Some("dead".into()),
        expires_at: 0,
        dpop_key: portside_atproto::DpopKey::generate().expect("key").secret(),
    };
    let agent = Agent::new(
        reqwest::Client::new(),
        session,
        Arc::new(MemoryStore::default()),
    )
    .expect("agent");
    let err = agent
        .get::<Value>("app.bsky.actor.getProfile", &[], None)
        .await
        .expect_err("expired");
    assert!(matches!(err, AtprotoError::SessionExpired));
}
