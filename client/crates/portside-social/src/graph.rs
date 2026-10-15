//! Friends are mutual follows (ADR-0021): you follow them and they follow you on Bluesky.

use portside_atproto::Agent;
use serde::Deserialize;
use serde_json::json;

use crate::error::{Result, SocialError};
use crate::models::{Person, ProfileWire, now_rfc3339, rkey};

/// Follows read when listing friends (100 per page), so very large follow lists stay quick.
const MAX_FOLLOW_PAGES: usize = 30;
/// Followers read for "Follows you".
const MAX_FOLLOWER_PAGES: usize = 3;

#[derive(Deserialize)]
struct FollowsPage {
    follows: Vec<ProfileWire>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Deserialize)]
struct FollowersPage {
    followers: Vec<ProfileWire>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Deserialize)]
struct ActorsPage {
    actors: Vec<ProfileWire>,
}

/// The signed-in account's profile, or anyone's.
pub async fn profile(agent: &Agent, actor: &str) -> Result<Person> {
    let p: ProfileWire = agent
        .get(
            "app.bsky.actor.getProfile",
            &[("actor", actor.to_owned())],
            None,
        )
        .await?;
    Ok(p.into())
}

/// Everyone you follow who follows you back, by name.
pub async fn friends(agent: &Agent) -> Result<Vec<Person>> {
    let mut friends = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_FOLLOW_PAGES {
        let mut query = vec![
            ("actor", agent.did().to_owned()),
            ("limit", "100".to_owned()),
        ];
        if let Some(c) = &cursor {
            query.push(("cursor", c.clone()));
        }
        let page: FollowsPage = agent.get("app.bsky.graph.getFollows", &query, None).await?;
        friends.extend(
            page.follows
                .into_iter()
                .filter(|p| !p.hidden())
                .map(Person::from)
                .filter(Person::is_friend),
        );
        cursor = page.cursor;
        if cursor.is_none() {
            break;
        }
    }
    sort_by_name(&mut friends);
    Ok(friends)
}

/// People who follow you and you don't follow back: following them makes them friends.
pub async fn follows_you(agent: &Agent) -> Result<Vec<Person>> {
    let mut people = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_FOLLOWER_PAGES {
        let mut query = vec![
            ("actor", agent.did().to_owned()),
            ("limit", "100".to_owned()),
        ];
        if let Some(c) = &cursor {
            query.push(("cursor", c.clone()));
        }
        let page: FollowersPage = agent
            .get("app.bsky.graph.getFollowers", &query, None)
            .await?;
        people.extend(
            page.followers
                .into_iter()
                .filter(|p| !p.hidden())
                .map(Person::from)
                .filter(|p| !p.following),
        );
        cursor = page.cursor;
        if cursor.is_none() {
            break;
        }
    }
    Ok(people)
}

/// People whose name or handle starts with `q`.
pub async fn search(agent: &Agent, q: &str) -> Result<Vec<Person>> {
    let q = q.trim().trim_start_matches('@');
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let page: ActorsPage = agent
        .get(
            "app.bsky.actor.searchActorsTypeahead",
            &[("q", q.to_owned()), ("limit", "10".to_owned())],
            None,
        )
        .await?;
    Ok(page
        .actors
        .into_iter()
        .filter(|p| !p.hidden() && p.did != agent.did())
        .map(Person::from)
        .collect())
}

/// Follows `did` ("Add friend"). They become a friend once they follow back.
pub async fn follow(agent: &Agent, did: &str) -> Result<Person> {
    check_did(did)?;
    let current = profile(agent, did).await?;
    if !current.following {
        let _: serde_json::Value = agent
            .post(
                "com.atproto.repo.createRecord",
                &json!({
                    "repo": agent.did(),
                    "collection": "app.bsky.graph.follow",
                    "record": {
                        "$type": "app.bsky.graph.follow",
                        "subject": did,
                        "createdAt": now_rfc3339(),
                    }
                }),
                None,
            )
            .await?;
    }
    Ok(Person {
        following: true,
        ..current
    })
}

/// Stops following `did` ("Remove friend").
pub async fn unfollow(agent: &Agent, did: &str) -> Result<Person> {
    check_did(did)?;
    let p: ProfileWire = agent
        .get(
            "app.bsky.actor.getProfile",
            &[("actor", did.to_owned())],
            None,
        )
        .await?;
    if let Some(uri) = p.viewer.as_ref().and_then(|v| v.following.clone()) {
        let key = rkey(&uri)
            .ok_or_else(|| SocialError::Invalid(format!("unexpected follow record {uri}")))?;
        let _: serde_json::Value = agent
            .post(
                "com.atproto.repo.deleteRecord",
                &json!({
                    "repo": agent.did(),
                    "collection": "app.bsky.graph.follow",
                    "rkey": key,
                }),
                None,
            )
            .await?;
    }
    Ok(Person {
        following: false,
        ..Person::from(p)
    })
}

fn sort_by_name(people: &mut [Person]) {
    people.sort_by_cached_key(|p| p.name().to_lowercase());
}

pub(crate) fn check_did(did: &str) -> Result<()> {
    if did.starts_with("did:") && did.len() < 2048 && !did.contains(char::is_whitespace) {
        Ok(())
    } else {
        Err(SocialError::Invalid(format!("{did} isn't a DID")))
    }
}
