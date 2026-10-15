//! HTTP behaviour against a local mock catalog host (synthetic data only).

use portside_api::{ApiError, Client, SnapshotResult};
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn summary(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "game_key": "synthetic-game",
        "title": "Synthetic Port",
        "type": "decompilation",
        "status": "playable",
        "os_status": [{ "os": "linux", "status": "playable", "source": "manifest" }],
        "stars": 42,
        "archived": false,
        "screenshot_url": null,
        "play_url": null,
        "updated_at": "2026-10-13T00:00:00.000Z"
    })
}

fn client(server: &MockServer) -> Client {
    Client::new(&server.uri(), "portside-desktop/test").expect("client")
}

#[tokio::test]
async fn port_reads_its_file() {
    let server = MockServer::start().await;
    let mut detail = summary("p-1");
    let extra = json!({
        "description": null, "repo": null, "homepage": null, "license": null,
        "last_commit": null, "upstream_score": null, "ratings": null, "video_url": null,
        "note": null, "status_notes": null, "manifest": null, "manifest_rev": null,
        "verified": false, "latest_releases": []
    });
    detail
        .as_object_mut()
        .expect("object")
        .extend(extra.as_object().expect("object").clone());
    Mock::given(method("GET"))
        .and(path("/v1/ports/p-1.json"))
        .and(header("user-agent", "portside-desktop/test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(detail))
        .expect(1)
        .mount(&server)
        .await;
    let port = client(&server).port("p-1").await.expect("port");
    assert_eq!(port.summary.id, "p-1");
}

#[tokio::test]
async fn ids_are_percent_encoded() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/ports/a%2Fb/releases.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&server)
        .await;
    let releases = client(&server).releases("a/b").await.expect("releases");
    assert!(releases.is_empty());
}

#[tokio::test]
async fn problem_details_become_status_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/ports/missing.json"))
        .respond_with(
            ResponseTemplate::new(404)
                .insert_header("content-type", "application/problem+json")
                .insert_header("x-request-id", "req-1")
                .set_body_json(json!({
                    "type": "about:blank",
                    "title": "Not Found",
                    "status": 404,
                    "detail": "No port with id missing"
                })),
        )
        .mount(&server)
        .await;

    let err = client(&server).port("missing").await.expect_err("404");
    assert!(err.is_not_found());
    match err {
        ApiError::Status {
            problem,
            request_id,
            ..
        } => {
            assert_eq!(request_id.as_deref(), Some("req-1"));
            assert_eq!(
                problem.and_then(|p| p.detail).as_deref(),
                Some("No port with id missing")
            );
        }
        other => panic!("unexpected error {other:?}"),
    }
}

#[tokio::test]
async fn snapshot_honours_etag() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/snapshot.json"))
        .and(header("if-none-match", "\"abc\""))
        .respond_with(ResponseTemplate::new(304).insert_header("etag", "\"abc\""))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/snapshot.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("etag", "\"def\"")
                .set_body_json(json!({ "rev": 7, "ports": [summary("p-2")] })),
        )
        .mount(&server)
        .await;

    let c = client(&server);
    let not_modified = c.snapshot(Some("\"abc\"")).await.expect("304");
    assert_eq!(
        not_modified,
        SnapshotResult::NotModified {
            etag: Some("\"abc\"".into())
        }
    );

    match c.snapshot(None).await.expect("snapshot") {
        SnapshotResult::Snapshot { etag, snapshot } => {
            assert_eq!(etag.as_deref(), Some("\"def\""));
            assert_eq!(snapshot.rev, 7);
            assert_eq!(snapshot.ports.len(), 1);
        }
        other => panic!("expected a snapshot, got {other:?}"),
    }
}

#[tokio::test]
async fn unreachable_host_is_a_network_error() {
    // Port 9 (discard) on localhost: nothing listens there in CI.
    let c = Client::new("http://127.0.0.1:9", "portside-desktop/test").expect("client");
    let err = c.snapshot(None).await.expect_err("no server");
    assert!(matches!(err, ApiError::Network(_)), "{err:?}");
}

#[test]
fn rejects_non_http_base_urls() {
    assert!(matches!(
        Client::new("file:///etc", "ua"),
        Err(ApiError::BaseUrl(_))
    ));
}

#[tokio::test]
async fn base_url_with_path_prefix() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/portside-catalog/v1/games/synthetic-game.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "game_key": "synthetic-game",
            "title": "Synthetic Game",
            "original_platforms": ["N64"],
            "ports": [summary("p-3")]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let c = Client::new(&format!("{}/portside-catalog", server.uri()), "ua").expect("client");
    let game = c.game("synthetic-game").await.expect("game");
    assert_eq!(game.ports.len(), 1);
}
