//! Local catalog: storage, search and sync against a mock catalog host (synthetic data only).

use portside_api::models::{CatalogSnapshot, OsStatus, PortSummary};
use portside_api::{Client, SearchParams};
use portside_catalog::{Catalog, SyncOutcome, sync};
use portside_core::{Os, PlayableStatus, PortType, StatusSource};
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn port(id: &str, title: &str, stars: Option<i64>, os: &[Os]) -> PortSummary {
    PortSummary {
        id: id.into(),
        game_key: format!("{id}-game"),
        title: title.into(),
        port_type: PortType::Decompilation,
        status: PlayableStatus::Playable,
        os_status: os
            .iter()
            .map(|&os| OsStatus {
                os,
                status: PlayableStatus::Playable,
                source: StatusSource::Manifest,
            })
            .collect(),
        stars,
        archived: false,
        screenshot_url: None,
        play_url: None,
        updated_at: "2026-10-13T00:00:00.000Z".into(),
    }
}

fn snapshot(rev: u64, ports: Vec<PortSummary>) -> CatalogSnapshot {
    CatalogSnapshot { rev, ports }
}

fn seeded() -> Catalog {
    let c = Catalog::open_in_memory().expect("open");
    c.replace_all(
        &snapshot(
            3,
            vec![
                port("a", "Example Racer", Some(10), &[Os::Linux]),
                port(
                    "b",
                    "Example Racer Deluxe",
                    Some(500),
                    &[Os::Windows, Os::Linux],
                ),
                port("c", "Example Puzzle", None, &[]),
                port("d", "Another Platformer", Some(42), &[Os::Macos]),
            ],
        ),
        Some("\"seed\""),
        100,
    )
    .expect("seed");
    c
}

fn ids(page: &portside_api::models::Page<PortSummary>) -> Vec<&str> {
    page.items.iter().map(|p| p.id.as_str()).collect()
}

#[test]
fn browse_orders_by_stars_with_unrated_last() {
    let c = seeded();
    let page = c.search(&SearchParams::default()).expect("search");
    assert_eq!(ids(&page), ["b", "d", "a", "c"]);
    assert_eq!(page.cursor, None);
    let state = c.state().expect("state");
    assert_eq!(
        (state.rev, state.port_count, state.last_synced_at),
        (Some(3), 4, Some(100))
    );
}

#[test]
fn text_search_matches_word_prefixes() {
    let c = seeded();
    let page = c
        .search(&SearchParams {
            q: Some("exam rac".into()),
            ..SearchParams::default()
        })
        .expect("search");
    let mut found = ids(&page);
    found.sort_unstable();
    assert_eq!(found, ["a", "b"]);
    // Odd input never becomes FTS syntax.
    c.search(&SearchParams {
        q: Some("\"NEAR( OR *".into()),
        ..SearchParams::default()
    })
    .expect("hostile query");
}

#[test]
fn filters_and_paging() {
    let c = seeded();
    let linux = c
        .search(&SearchParams {
            os: Some(Os::Linux),
            ..SearchParams::default()
        })
        .expect("search");
    assert_eq!(ids(&linux), ["b", "a"]);

    let first = c
        .search(&SearchParams {
            limit: Some(3),
            ..SearchParams::default()
        })
        .expect("page 1");
    assert_eq!(ids(&first), ["b", "d", "a"]);
    let second = c
        .search(&SearchParams {
            limit: Some(3),
            cursor: first.cursor.clone(),
            ..SearchParams::default()
        })
        .expect("page 2");
    assert_eq!(ids(&second), ["c"]);
    assert_eq!(second.cursor, None);
}

#[test]
fn replacing_updates_the_search_index() {
    let c = seeded();
    c.replace_all(
        &snapshot(4, vec![port("a", "Renamed Kart", Some(10), &[])]),
        None,
        200,
    )
    .expect("replace");
    let racer = c
        .search(&SearchParams {
            q: Some("racer".into()),
            ..SearchParams::default()
        })
        .expect("search");
    assert!(racer.items.is_empty(), "old title and removed port gone");
    let kart = c
        .search(&SearchParams {
            q: Some("kart".into()),
            ..SearchParams::default()
        })
        .expect("search");
    assert_eq!(ids(&kart), ["a"]);
    assert!(c.get("b").expect("get").is_none());
    let state = c.state().expect("state");
    assert_eq!(state.rev, Some(4));
    assert_eq!(state.etag, None);
}

fn snapshot_json(rev: u64, ports: Vec<PortSummary>) -> serde_json::Value {
    serde_json::to_value(snapshot(rev, ports)).expect("json")
}

#[tokio::test]
async fn sync_downloads_then_uses_the_etag() {
    let server = MockServer::start().await;
    let api = Client::new(&server.uri(), "test").expect("client");
    let c = Catalog::open_in_memory().expect("open");

    // First run: no ETag sent.
    Mock::given(method("GET"))
        .and(path("/v1/snapshot.json"))
        .and(|req: &wiremock::Request| !req.headers.contains_key("if-none-match"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("etag", "\"v5\"")
                .set_body_json(snapshot_json(
                    5,
                    vec![port("a", "Example Racer", Some(1), &[])],
                )),
        )
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        sync(&c, &api).await.expect("first"),
        SyncOutcome::Updated { rev: 5, ports: 1 }
    );
    assert_eq!(c.state().expect("state").etag.as_deref(), Some("\"v5\""));

    // Unchanged: 304.
    Mock::given(method("GET"))
        .and(path("/v1/snapshot.json"))
        .and(header("if-none-match", "\"v5\""))
        .respond_with(ResponseTemplate::new(304))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    assert_eq!(
        sync(&c, &api).await.expect("304"),
        SyncOutcome::Unchanged { rev: 5 }
    );

    // Changed: the whole file again, replacing the copy.
    Mock::given(method("GET"))
        .and(path("/v1/snapshot.json"))
        .and(header("if-none-match", "\"v5\""))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("etag", "\"v6\"")
                .set_body_json(json!({
                    "rev": 6,
                    "ports": [serde_json::to_value(port("b", "Example Puzzle", None, &[])).expect("json")]
                })),
        )
        .mount(&server)
        .await;
    assert_eq!(
        sync(&c, &api).await.expect("changed"),
        SyncOutcome::Updated { rev: 6, ports: 1 }
    );
    assert!(c.get("a").expect("get").is_none());
    assert!(c.get("b").expect("get").is_some());
    assert_eq!(c.state().expect("state").etag.as_deref(), Some("\"v6\""));
}

#[tokio::test]
async fn sync_same_rev_under_a_new_etag_keeps_the_copy() {
    let server = MockServer::start().await;
    let api = Client::new(&server.uri(), "test").expect("client");
    let c = seeded();
    Mock::given(method("GET"))
        .and(path("/v1/snapshot.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("etag", "\"redeployed\"")
                .set_body_json(snapshot_json(3, vec![])),
        )
        .mount(&server)
        .await;
    assert_eq!(
        sync(&c, &api).await.expect("sync"),
        SyncOutcome::Unchanged { rev: 3 }
    );
    let state = c.state().expect("state");
    assert_eq!(state.port_count, 4);
    assert_eq!(state.etag.as_deref(), Some("\"redeployed\""));
}

#[tokio::test]
async fn sync_offline_keeps_the_copy() {
    let c = seeded();
    let api = Client::new("http://127.0.0.1:9", "test").expect("client");
    assert!(sync(&c, &api).await.is_err());
    assert_eq!(c.state().expect("state").port_count, 4);
}

#[test]
fn reopening_a_file_keeps_data() {
    let dir = std::env::temp_dir().join(format!("portside-catalog-test-{}", std::process::id()));
    let file = dir.join("nested").join("portside.db");
    {
        let c = Catalog::open(&file).expect("open");
        c.replace_all(&snapshot(1, vec![port("a", "Example", None, &[])]), None, 1)
            .expect("seed");
    }
    let c = Catalog::open(&file).expect("reopen");
    assert_eq!(c.state().expect("state").port_count, 1);
    drop(c);
    let _ = std::fs::remove_dir_all(dir);
}
