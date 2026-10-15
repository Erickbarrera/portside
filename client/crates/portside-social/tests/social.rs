//! Friends, chat, invites and boards against a mock account host (synthetic accounts).

use std::sync::Arc;

use portside_atproto::{Agent, DpopKey, MemoryStore, Session};
use portside_social::boards::{self, BoardQuery, SubForum};
use portside_social::chat::{Chat, ChatEvent};
use portside_social::graph;
use portside_social::invite::InviteLinks;
use serde_json::{Value, json};
use wiremock::matchers::{body_partial_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ME: &str = "did:plc:me";
const SITE: &str = "https://example.github.io/portside-catalog/";

fn agent(server: &MockServer) -> Agent {
    let session = Session {
        did: ME.into(),
        handle: Some("me.test".into()),
        pds: server.uri(),
        issuer: server.uri(),
        token_endpoint: format!("{}/oauth/token", server.uri()),
        revocation_endpoint: None,
        client_id: "http://localhost".into(),
        scope: "atproto transition:generic transition:chat.bsky".into(),
        access_token: "tok".into(),
        refresh_token: None,
        expires_at: u64::MAX / 2,
        dpop_key: DpopKey::generate().expect("key").secret(),
    };
    Agent::new(
        reqwest::Client::new(),
        session,
        Arc::new(MemoryStore::default()),
    )
    .expect("agent")
}

fn person(did: &str, following: bool, followed_by: bool) -> Value {
    let mut viewer = json!({});
    if following {
        viewer["following"] = json!(format!("at://{ME}/app.bsky.graph.follow/f{}", &did[8..]));
    }
    if followed_by {
        viewer["followedBy"] = json!(format!("at://{did}/app.bsky.graph.follow/x"));
    }
    json!({
        "did": did,
        "handle": format!("{}.test", &did[8..]),
        "displayName": format!("Example {}", &did[8..]),
        "viewer": viewer
    })
}

fn chat() -> Chat {
    Chat::new(InviteLinks::new(SITE).expect("links"))
}

#[tokio::test]
async fn friends_are_mutual_follows_across_pages() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/xrpc/app.bsky.graph.getFollows"))
        .and(query_param("actor", ME))
        .and(query_param("cursor", "p2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "follows": [person("did:plc:bob", true, true)]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/xrpc/app.bsky.graph.getFollows"))
        .and(query_param("actor", ME))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "follows": [person("did:plc:zed", true, true), person("did:plc:one", true, false)],
            "cursor": "p2"
        })))
        .mount(&server)
        .await;
    let friends = graph::friends(&agent(&server)).await.expect("friends");
    let dids: Vec<&str> = friends.iter().map(|p| p.did.as_str()).collect();
    assert_eq!(dids, vec!["did:plc:bob", "did:plc:zed"]);
}

#[tokio::test]
async fn adding_a_friend_follows_them() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/xrpc/app.bsky.actor.getProfile"))
        .respond_with(ResponseTemplate::new(200).set_body_json(person("did:plc:bob", false, true)))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.repo.createRecord"))
        .and(body_partial_json(json!({
            "repo": ME,
            "collection": "app.bsky.graph.follow",
            "record": { "subject": "did:plc:bob" }
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "uri": "at://x", "cid": "c" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    let bob = graph::follow(&agent(&server), "did:plc:bob")
        .await
        .expect("follow");
    assert!(bob.is_friend());
}

#[tokio::test]
async fn invites_go_through_the_chat_proxy_with_a_link_facet() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/xrpc/chat.bsky.convo.getConvoForMembers"))
        .and(header("atproto-proxy", "did:web:api.bsky.chat#bsky_chat"))
        .and(query_param("members", "did:plc:bob"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "convo": {
                "id": "c1", "rev": "r1", "muted": false, "unreadCount": 0,
                "members": [person(ME, false, false), person("did:plc:bob", true, true)]
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xrpc/chat.bsky.convo.sendMessage"))
        .and(header("atproto-proxy", "did:web:api.bsky.chat#bsky_chat"))
        .and(body_partial_json(json!({ "convoId": "c1" })))
        .respond_with(|req: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&req.body).expect("json");
            let text = body["message"]["text"].as_str().expect("text").to_owned();
            let facet = &body["message"]["facets"][0];
            let start = facet["index"]["byteStart"].as_u64().expect("start") as usize;
            let end = facet["index"]["byteEnd"].as_u64().expect("end") as usize;
            assert_eq!(
                &text[start..end],
                facet["features"][0]["uri"].as_str().expect("uri")
            );
            ResponseTemplate::new(200).set_body_json(json!({
                "$type": "chat.bsky.convo.defs#messageView",
                "id": "m1", "rev": "r2", "text": text, "facets": body["message"]["facets"],
                "sender": { "did": ME }, "sentAt": "2026-10-14T12:00:00.000Z"
            }))
        })
        .expect(1)
        .mount(&server)
        .await;
    let (convo, message) = chat()
        .invite(
            &agent(&server),
            "did:plc:bob",
            "example-port",
            "Example Game",
        )
        .await
        .expect("invite");
    assert_eq!(convo.members.len(), 1);
    assert_eq!(convo.members[0].did, "did:plc:bob");
    assert_eq!(message.invite.expect("invite").port_id, "example-port");
}

#[tokio::test]
async fn messages_come_oldest_first_and_the_log_reports_new_ones() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/xrpc/chat.bsky.convo.getMessages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "messages": [
                { "$type": "chat.bsky.convo.defs#messageView", "id": "m2", "rev": "2",
                  "text": "second", "sender": { "did": "did:plc:bob" }, "sentAt": "2026-10-14T12:01:00Z" },
                { "$type": "chat.bsky.convo.defs#deletedMessageView", "id": "m1", "rev": "1",
                  "sender": { "did": ME }, "sentAt": "2026-10-14T12:00:00Z" },
                { "$type": "chat.bsky.convo.defs#systemMessageView", "id": "s", "rev": "0",
                  "sentAt": "2026-10-14T11:00:00Z", "data": {} }
            ],
            "cursor": "older"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/xrpc/chat.bsky.convo.getLog"))
        .and(query_param("cursor", "r1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "cursor": "r3",
            "logs": [
                { "$type": "chat.bsky.convo.defs#logCreateMessage", "rev": "r2", "convoId": "c1",
                  "message": { "$type": "chat.bsky.convo.defs#messageView", "id": "m3", "rev": "r2",
                    "text": "join me https://example.github.io/portside-catalog/catalog/?q=X&port=p1",
                    "sender": { "did": "did:plc:bob" }, "sentAt": "2026-10-14T12:02:00Z" } },
                { "$type": "chat.bsky.convo.defs#logReadMessage", "rev": "r3", "convoId": "c1",
                  "message": { "id": "m3" } }
            ]
        })))
        .mount(&server)
        .await;
    let agent = agent(&server);
    let page = chat().messages(&agent, "c1", None).await.expect("messages");
    let ids: Vec<&str> = page.messages.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["m1", "m2"]);
    assert!(page.messages[0].deleted);
    assert_eq!(page.cursor.as_deref(), Some("older"));

    let batch = chat().poll(&agent, Some("r1")).await.expect("poll");
    assert_eq!(batch.cursor.as_deref(), Some("r3"));
    match &batch.events[0] {
        ChatEvent::Message { convo_id, message } => {
            assert_eq!(convo_id, "c1");
            assert_eq!(
                message.invite.as_ref().map(|i| i.port_id.as_str()),
                Some("p1")
            );
        }
        other => panic!("unexpected {other:?}"),
    }
    assert!(matches!(batch.events[1], ChatEvent::Changed { .. }));
}

fn post(uri: &str, author: &str, text: &str, reply: Option<(&str, &str)>, tags: &[&str]) -> Value {
    let mut record = json!({
        "$type": "app.bsky.feed.post", "text": text, "tags": tags,
        "createdAt": format!("2026-10-14T12:00:{:02}Z", uri.len() % 60)
    });
    if let Some((root, parent)) = reply {
        record["reply"] = json!({
            "root": { "uri": root, "cid": "c" }, "parent": { "uri": parent, "cid": "c" }
        });
    }
    json!({
        "uri": uri, "cid": format!("cid-{uri}"), "author": person(author, false, false),
        "record": record, "replyCount": 1, "likeCount": 2, "indexedAt": "2026-10-14T12:00:00Z"
    })
}

#[tokio::test]
async fn a_board_groups_posts_by_thread_and_fetches_missing_first_posts() {
    let server = MockServer::start().await;
    let tags = ["portsideapp", "portsideapp-p1", "psapp-help"];
    Mock::given(method("GET"))
        .and(path("/xrpc/app.bsky.feed.searchPosts"))
        .and(query_param("q", "#portsideapp-p1"))
        .and(query_param("sort", "latest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "posts": [
                post("at://did:plc:b/app.bsky.feed.post/r1", "did:plc:b", "a reply",
                     Some(("at://did:plc:a/app.bsky.feed.post/t1", "at://did:plc:a/app.bsky.feed.post/t1")), &tags),
                post("at://did:plc:c/app.bsky.feed.post/t2", "did:plc:c", "Second thread\n\nBody", None, &tags),
                // The hashtag typed in the text, not a hidden tag: not a board post.
                post("at://did:plc:d/app.bsky.feed.post/x1", "did:plc:d", "#portsideapp-p1 unrelated", None, &[]),
            ],
            "cursor": "next"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/xrpc/app.bsky.feed.getPosts"))
        .and(query_param("uris", "at://did:plc:a/app.bsky.feed.post/t1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "posts": [post("at://did:plc:a/app.bsky.feed.post/t1", "did:plc:a", "Bumped thread\n\nBody (1/2)", None, &tags)]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let page = boards::board(
        &agent(&server),
        &BoardQuery {
            port_id: Some("p1".into()),
            sub_forum: Some(SubForum::Help),
            ..BoardQuery::default()
        },
    )
    .await
    .expect("board");
    let titles: Vec<&str> = page.threads.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, vec!["Bumped thread", "Second thread"]);
    assert_eq!(page.threads[0].port_id.as_deref(), Some("p1"));
    assert_eq!(page.threads[0].sub_forum, Some(SubForum::Help));
    assert_eq!(page.cursor.as_deref(), Some("next"));
}

#[tokio::test]
async fn a_thread_stitches_its_parts_and_lists_replies() {
    let server = MockServer::start().await;
    let tags = [
        "portsideapp",
        "portsideapp-p1",
        "psapp-general",
        "unrelated",
    ];
    let root = "at://did:plc:a/app.bsky.feed.post/t1";
    let part2 = "at://did:plc:a/app.bsky.feed.post/t1b";
    let other = "at://did:plc:b/app.bsky.feed.post/r1";
    let answer = "at://did:plc:a/app.bsky.feed.post/r2";
    let tvp = |p: Value, replies: Vec<Value>| json!({ "$type": "app.bsky.feed.defs#threadViewPost", "post": p, "replies": replies });
    let thread = tvp(
        post(
            root,
            "did:plc:a",
            "Title here\n\nFirst half (1/2)",
            None,
            &tags,
        ),
        vec![
            tvp(
                post(
                    part2,
                    "did:plc:a",
                    "second half (2/2)",
                    Some((root, root)),
                    &tags,
                ),
                vec![],
            ),
            tvp(
                post(other, "did:plc:b", "Nice", Some((root, root)), &tags),
                vec![tvp(
                    post(answer, "did:plc:a", "Thanks", Some((root, other)), &tags),
                    vec![],
                )],
            ),
            json!({ "$type": "app.bsky.feed.defs#blockedPost", "uri": "at://x", "blocked": true }),
        ],
    );
    Mock::given(method("GET"))
        .and(path("/xrpc/app.bsky.feed.getPostThread"))
        .and(query_param("uri", root))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "thread": thread })))
        .mount(&server)
        .await;
    let view = boards::thread(&agent(&server), root).await.expect("thread");
    assert_eq!(view.title, "Title here");
    assert_eq!(view.first.text, "First half\nsecond half");
    assert_eq!(view.replies.len(), 2);
    let thanks = view
        .replies
        .iter()
        .find(|r| r.text == "Thanks")
        .expect("thanks");
    assert_eq!(
        thanks.reply_to.as_ref().map(|p| p.did.as_str()),
        Some("did:plc:b")
    );
    assert_eq!(
        view.tags,
        vec!["portsideapp", "portsideapp-p1", "psapp-general"]
    );
    assert_eq!(view.web_url, "https://bsky.app/profile/a.test/post/t1");
}

#[tokio::test]
async fn a_long_first_post_becomes_a_numbered_chain() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.repo.createRecord"))
        .and(body_partial_json(json!({
            "collection": "app.bsky.feed.post",
            "record": { "tags": ["portsideapp", "portsideapp-p1", "psapp-bugs"] }
        })))
        .respond_with(|req: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&req.body).expect("json");
            let text = body["record"]["text"].as_str().expect("text");
            assert!(text.chars().count() <= 300);
            let n = text
                .rsplit('(')
                .next()
                .expect("marker")
                .chars()
                .next()
                .expect("k");
            ResponseTemplate::new(200).set_body_json(json!({
                "uri": format!("at://{ME}/app.bsky.feed.post/p{n}"), "cid": format!("c{n}")
            }))
        })
        .expect(3)
        .mount(&server)
        .await;
    let body = "word ".repeat(140);
    let first = boards::post_thread(&agent(&server), "p1", SubForum::Bugs, "Crash", &body)
        .await
        .expect("post");
    assert_eq!(first.uri, format!("at://{ME}/app.bsky.feed.post/p1"));
    let requests = server.received_requests().await.expect("requests");
    let third: Value = serde_json::from_slice(&requests[2].body).expect("json");
    assert_eq!(third["record"]["reply"]["root"]["uri"], first.uri);
    assert_eq!(third["record"]["reply"]["parent"]["cid"], "c2");
}
