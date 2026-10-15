//! Discussion boards on Bluesky (docs/specs/boards.md): threads are ordinary posts with
//! hidden tags, read through the account host's index (`app.bsky.feed.searchPosts`).

use std::collections::HashMap;

use portside_atproto::Agent;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::error::{Result, SocialError};
use crate::models::{CreatedRecord, Person, ProfileWire, StrongRef, now_rfc3339};
use crate::richtext::{MAX_GRAPHEMES, link_facets, number_parts, part_marker, split};

/// On every Portside board post. Distinctive on purpose: plain `#portside` is used for
/// plenty of unrelated things on Bluesky.
pub const SITE_TAG: &str = "portsideapp";
/// Prefix of the sub-forum tags.
const SUB_FORUM_PREFIX: &str = "psapp-";
/// Bluesky allows 8 tags of up to 64 characters each.
const MAX_TAGS: usize = 8;
const MAX_TAG_LEN: usize = 64;
/// Longest title, in characters.
pub const MAX_TITLE: usize = 120;
/// Longest first post or reply before it is refused (split into up to ten posts).
pub const MAX_BODY_GRAPHEMES: usize = 2_800;

/// Sub-forums of a game's board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SubForum {
    General,
    Help,
    Bugs,
    Mods,
    Multiplayer,
}

impl SubForum {
    pub const ALL: [SubForum; 5] = [
        SubForum::General,
        SubForum::Help,
        SubForum::Bugs,
        SubForum::Mods,
        SubForum::Multiplayer,
    ];

    pub fn tag(self) -> &'static str {
        match self {
            SubForum::General => "psapp-general",
            SubForum::Help => "psapp-help",
            SubForum::Bugs => "psapp-bugs",
            SubForum::Mods => "psapp-mods",
            SubForum::Multiplayer => "psapp-mp",
        }
    }

    pub fn from_tags(tags: &[String]) -> Option<SubForum> {
        SubForum::ALL
            .into_iter()
            .find(|s| tags.iter().any(|t| t == s.tag()))
    }
}

/// Portside's own boards, not tied to a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SiteBoard {
    /// Help with Portside itself.
    Help,
    /// Feature requests and ideas.
    Features,
}

impl SiteBoard {
    pub const ALL: [SiteBoard; 2] = [SiteBoard::Help, SiteBoard::Features];

    pub fn tag(self) -> &'static str {
        match self {
            SiteBoard::Help => "portsideapp-site-help",
            SiteBoard::Features => "portsideapp-site-features",
        }
    }

    pub fn from_tags(tags: &[String]) -> Option<SiteBoard> {
        SiteBoard::ALL
            .into_iter()
            .find(|b| tags.iter().any(|t| t == b.tag()))
    }
}

/// `portsideapp-<port id>`; ids over 52 characters become the first 43, a dash and the
/// first 8 hex digits of the id's SHA-256, so the tag stays within 64 characters.
pub fn port_tag(port_id: &str) -> String {
    if port_id.chars().count() <= 52 {
        format!("{SITE_TAG}-{port_id}")
    } else {
        let head: String = port_id.chars().take(43).collect();
        let hash = hex::encode(Sha256::digest(port_id.as_bytes()));
        format!("{SITE_TAG}-{head}-{}", &hash[..8])
    }
}

/// Which posts to show: all of Portside, a game's board, or one of its sub-forums.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct BoardQuery {
    pub port_id: Option<String>,
    pub sub_forum: Option<SubForum>,
    /// One of Portside's own boards (ignored when `port_id` is set).
    pub site_board: Option<SiteBoard>,
    /// Words to search for within the board.
    pub q: Option<String>,
    #[serde(default)]
    pub sort: BoardSort,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum BoardSort {
    /// Latest activity first: a new reply from Portside bumps its thread.
    #[default]
    Latest,
    Top,
}

impl BoardQuery {
    pub fn tags(&self) -> Vec<String> {
        let mut tags = vec![SITE_TAG.to_owned()];
        if let Some(id) = &self.port_id {
            tags.push(port_tag(id));
            if let Some(sub) = self.sub_forum {
                tags.push(sub.tag().to_owned());
            }
        } else if let Some(board) = self.site_board {
            tags.push(board.tag().to_owned());
        }
        tags
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ThreadSummary {
    pub root: StrongRef,
    pub title: String,
    pub author: Person,
    pub created_at: String,
    /// Newest post seen for this thread.
    pub last_activity: String,
    pub reply_count: u32,
    pub like_count: u32,
    pub port_id: Option<String>,
    pub sub_forum: Option<SubForum>,
    pub site_board: Option<SiteBoard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct BoardPage {
    pub threads: Vec<ThreadSummary>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct BoardPost {
    pub post: StrongRef,
    pub author: Person,
    /// For the first post, the stitched text of all its parts without the title.
    pub text: String,
    pub created_at: String,
    pub like_count: u32,
    pub reply_count: u32,
    /// Who this answers, when it isn't the first post.
    pub reply_to: Option<Person>,
    /// Image thumbnails.
    pub images: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ThreadView {
    pub title: String,
    pub first: BoardPost,
    /// Oldest first.
    pub replies: Vec<BoardPost>,
    /// Board tags of the thread, repeated on replies written from Portside.
    pub tags: Vec<String>,
    pub port_id: Option<String>,
    pub sub_forum: Option<SubForum>,
    pub site_board: Option<SiteBoard>,
    /// Bluesky address of the first post, for "Open in Bluesky".
    pub web_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostWire {
    uri: String,
    cid: String,
    author: ProfileWire,
    record: Value,
    #[serde(default)]
    reply_count: Option<u32>,
    #[serde(default)]
    like_count: Option<u32>,
    #[serde(default)]
    embed: Option<Value>,
    #[serde(default)]
    indexed_at: String,
}

impl PostWire {
    fn text(&self) -> &str {
        self.record
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    fn created_at(&self) -> String {
        self.record
            .get("createdAt")
            .and_then(Value::as_str)
            .map_or_else(|| self.indexed_at.clone(), str::to_owned)
    }

    fn has_site_tag(&self) -> bool {
        self.tags().iter().any(|t| t == SITE_TAG)
    }

    fn tags(&self) -> Vec<String> {
        self.record
            .get("tags")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn reply_ref(&self, which: &str) -> Option<String> {
        self.record
            .get("reply")?
            .get(which)?
            .get("uri")?
            .as_str()
            .map(str::to_owned)
    }

    fn root_uri(&self) -> String {
        self.reply_ref("root").unwrap_or_else(|| self.uri.clone())
    }

    fn strong_ref(&self) -> StrongRef {
        StrongRef {
            uri: self.uri.clone(),
            cid: self.cid.clone(),
        }
    }

    fn images(&self) -> Vec<String> {
        let embed = match &self.embed {
            Some(e)
                if e.get("$type").and_then(Value::as_str)
                    == Some("app.bsky.embed.recordWithMedia#view") =>
            {
                e.get("media")
            }
            other => other.as_ref(),
        };
        embed
            .filter(|e| {
                e.get("$type").and_then(Value::as_str) == Some("app.bsky.embed.images#view")
            })
            .and_then(|e| e.get("images")?.as_array())
            .map(|imgs| {
                imgs.iter()
                    .filter_map(|i| i.get("thumb")?.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Splits a first post's text into its title (first line) and body.
pub fn title_and_body(text: &str) -> (String, String) {
    let text = text.trim();
    match text.split_once('\n') {
        Some((first, rest)) if first.chars().count() <= MAX_TITLE => {
            (first.trim().to_owned(), rest.trim().to_owned())
        }
        _ => {
            let title: String = text.chars().take(80).collect();
            let title = if title.len() < text.len() {
                format!("{}…", title.trim_end())
            } else {
                title
            };
            (title, text.to_owned())
        }
    }
}

/// Whether a tag is one of Portside's board tags.
fn is_board_tag(tag: &str) -> bool {
    tag.starts_with(SITE_TAG) || tag.starts_with(SUB_FORUM_PREFIX)
}

/// The port id behind a `portsideapp-…` tag, unless it was shortened.
fn port_from_tags(tags: &[String]) -> Option<String> {
    let prefix = format!("{SITE_TAG}-");
    tags.iter()
        .filter_map(|t| t.strip_prefix(prefix.as_str()))
        .find(|id| !id.starts_with("site-"))
        .map(str::to_owned)
}

/// Bluesky web address of a post.
pub fn web_url(uri: &str, handle: &str) -> String {
    let rkey = uri.rsplit('/').next().unwrap_or_default();
    format!("https://bsky.app/profile/{handle}/post/{rkey}")
}

#[derive(Deserialize)]
struct SearchPage {
    posts: Vec<PostWire>,
    #[serde(default)]
    cursor: Option<String>,
}

/// A page of threads, grouped by their first post and ordered by latest activity.
pub async fn board(agent: &Agent, query: &BoardQuery) -> Result<BoardPage> {
    let tags = query.tags();
    // The board's own tag (game or site board), not the sub-forum's: it is the rarer one.
    let board_tag = tags.get(1).cloned().unwrap_or_else(|| SITE_TAG.to_owned());
    let words = query.q.as_deref().map(str::trim).filter(|w| !w.is_empty());
    // `q` is required; a hashtag search matches the hidden `tags` too.
    let q = match words {
        Some(w) => format!("#{board_tag} {w}"),
        None => format!("#{board_tag}"),
    };
    let mut params = vec![
        ("q", q),
        (
            "sort",
            match query.sort {
                BoardSort::Latest => "latest",
                BoardSort::Top => "top",
            }
            .to_owned(),
        ),
        ("limit", "100".to_owned()),
    ];
    params.extend(tags.iter().map(|t| ("tag", t.clone())));
    if let Some(c) = &query.cursor {
        params.push(("cursor", c.clone()));
    }
    let page: SearchPage = agent
        .get("app.bsky.feed.searchPosts", &params, None)
        .await?;

    // Group by thread root, keeping the order each root was first seen.
    let mut order: Vec<String> = Vec::new();
    let mut last_seen: HashMap<String, String> = HashMap::new();
    let mut roots: HashMap<String, PostWire> = HashMap::new();
    for post in page.posts {
        // Only posts written from Portside carry the tag as a hidden tag; someone typing
        // the hashtag in a post's text doesn't put it on a board.
        if post.author.hidden() || !post.has_site_tag() {
            continue;
        }
        let root = post.root_uri();
        let created = post.created_at();
        match last_seen.get_mut(&root) {
            Some(seen) => {
                if created > *seen {
                    *seen = created;
                }
            }
            None => {
                order.push(root.clone());
                last_seen.insert(root.clone(), created);
            }
        }
        if post.uri == root {
            roots.insert(root, post);
        }
    }
    let missing: Vec<String> = order
        .iter()
        .filter(|r| !roots.contains_key(*r))
        .cloned()
        .collect();
    for chunk in missing.chunks(25) {
        #[derive(Deserialize)]
        struct Posts {
            posts: Vec<PostWire>,
        }
        let query: Vec<(&str, String)> = chunk.iter().map(|u| ("uris", u.clone())).collect();
        let found: Posts = agent.get("app.bsky.feed.getPosts", &query, None).await?;
        for post in found.posts {
            roots.insert(post.uri.clone(), post);
        }
    }

    let threads = order
        .into_iter()
        .filter_map(|uri| {
            let root = roots.remove(&uri)?;
            if root.author.hidden() || !root.has_site_tag() {
                return None;
            }
            let tags = root.tags();
            let text = part_marker(root.text()).map_or(root.text(), |(_, _, t)| t);
            let (title, _) = title_and_body(text);
            Some(ThreadSummary {
                root: root.strong_ref(),
                title,
                created_at: root.created_at(),
                last_activity: last_seen.remove(&uri).unwrap_or_default(),
                reply_count: root.reply_count.unwrap_or(0),
                like_count: root.like_count.unwrap_or(0),
                port_id: port_from_tags(&tags),
                sub_forum: SubForum::from_tags(&tags),
                site_board: SiteBoard::from_tags(&tags),
                author: root.author.into(),
            })
        })
        .collect();
    Ok(BoardPage {
        threads,
        cursor: page.cursor,
    })
}

/// A whole thread: the first post (its "1/n" parts stitched together) and every reply,
/// oldest first.
pub async fn thread(agent: &Agent, uri: &str) -> Result<ThreadView> {
    #[derive(Deserialize)]
    struct Thread {
        thread: Value,
    }
    let found: Thread = agent
        .get(
            "app.bsky.feed.getPostThread",
            &[
                ("uri", uri.to_owned()),
                ("depth", "50".to_owned()),
                ("parentHeight", "0".to_owned()),
            ],
            None,
        )
        .await?;
    let mut posts = Vec::new();
    collect(&found.thread, &mut posts);
    let Some(root) = posts.first().cloned() else {
        return Err(SocialError::Invalid(
            "This thread was deleted or isn't visible to you.".to_owned(),
        ));
    };
    let by_uri: HashMap<String, PostWire> =
        posts.iter().map(|p| (p.uri.clone(), p.clone())).collect();

    // Stitch the author's own "(k/n)" continuation chain.
    let mut text = root.text().to_owned();
    let mut chain = vec![root.uri.clone()];
    if let Some((1, n, first)) = part_marker(root.text()) {
        text = first.to_owned();
        let mut parent = root.uri.clone();
        for k in 2..=n {
            let next = posts.iter().find(|p| {
                p.author.did == root.author.did
                    && p.reply_ref("parent").as_deref() == Some(parent.as_str())
                    && part_marker(p.text()).is_some_and(|(pk, pn, _)| pk == k && pn == n)
            });
            let Some(next) = next else { break };
            if let Some((_, _, part)) = part_marker(next.text()) {
                text.push('\n');
                text.push_str(part);
            }
            parent = next.uri.clone();
            chain.push(next.uri.clone());
        }
    }
    let (title, body) = title_and_body(&text);
    let tags: Vec<String> = root
        .tags()
        .into_iter()
        .filter(|t| is_board_tag(t))
        .take(MAX_TAGS)
        .collect();

    let mut replies: Vec<BoardPost> = posts
        .iter()
        .filter(|p| !chain.contains(&p.uri) && !p.author.hidden())
        .map(|p| {
            let parent = p.reply_ref("parent");
            let reply_to = parent
                .filter(|parent| !chain.contains(parent))
                .and_then(|parent| by_uri.get(&parent))
                .map(|parent| Person::from(parent.author.clone()));
            board_post(p, p.text().to_owned(), reply_to)
        })
        .collect();
    replies.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    Ok(ThreadView {
        title,
        web_url: web_url(&root.uri, &root.author.handle),
        port_id: port_from_tags(&tags),
        sub_forum: SubForum::from_tags(&tags),
        site_board: SiteBoard::from_tags(&tags),
        first: board_post(&root, body, None),
        replies,
        tags,
    })
}

fn board_post(p: &PostWire, text: String, reply_to: Option<Person>) -> BoardPost {
    BoardPost {
        post: p.strong_ref(),
        author: p.author.clone().into(),
        text,
        created_at: p.created_at(),
        like_count: p.like_count.unwrap_or(0),
        reply_count: p.reply_count.unwrap_or(0),
        reply_to,
        images: p.images(),
    }
}

/// Every visible post of a `threadViewPost` tree, parents before children.
fn collect(node: &Value, out: &mut Vec<PostWire>) {
    if node.get("$type").and_then(Value::as_str) != Some("app.bsky.feed.defs#threadViewPost") {
        return; // notFoundPost, blockedPost
    }
    if let Some(post) = node
        .get("post")
        .and_then(|p| serde_json::from_value::<PostWire>(p.clone()).ok())
    {
        out.push(post);
    }
    if let Some(replies) = node.get("replies").and_then(Value::as_array) {
        for reply in replies {
            collect(reply, out);
        }
    }
}

/// Starts a thread on a game's board. Returns its first post.
pub async fn post_thread(
    agent: &Agent,
    port_id: &str,
    sub_forum: SubForum,
    title: &str,
    body: &str,
) -> Result<StrongRef> {
    let tags = vec![
        SITE_TAG.to_owned(),
        port_tag(port_id),
        sub_forum.tag().to_owned(),
    ];
    start_thread(agent, &tags, title, body).await
}

/// Starts a thread on one of Portside's own boards (help, feature requests).
pub async fn post_site_thread(
    agent: &Agent,
    board: SiteBoard,
    title: &str,
    body: &str,
) -> Result<StrongRef> {
    let tags = vec![SITE_TAG.to_owned(), board.tag().to_owned()];
    start_thread(agent, &tags, title, body).await
}

async fn start_thread(
    agent: &Agent,
    tags: &[String],
    title: &str,
    body: &str,
) -> Result<StrongRef> {
    let title = title.trim();
    if title.is_empty() || title.contains('\n') || title.chars().count() > MAX_TITLE {
        return Err(SocialError::Invalid(format!(
            "Give the thread a title of up to {MAX_TITLE} characters on one line."
        )));
    }
    let text = if body.trim().is_empty() {
        title.to_owned()
    } else {
        format!("{title}\n\n{}", body.trim())
    };
    write_chain(agent, &text, tags, None).await
}

/// Replies in a thread. `tags` are the thread's board tags (only `portsideapp…` and
/// `psapp-…` are kept), so the reply bumps the thread on its board.
pub async fn reply(
    agent: &Agent,
    root: &StrongRef,
    parent: &StrongRef,
    tags: &[String],
    text: &str,
) -> Result<StrongRef> {
    if text.trim().is_empty() {
        return Err(SocialError::Invalid("The reply is empty.".to_owned()));
    }
    let tags: Vec<String> = tags
        .iter()
        .filter(|t| is_board_tag(t) && t.len() <= MAX_TAG_LEN && !t.contains(char::is_whitespace))
        .take(MAX_TAGS)
        .cloned()
        .collect();
    write_chain(
        agent,
        text.trim(),
        &tags,
        Some((root.clone(), parent.clone())),
    )
    .await
}

/// Writes `text` as one post, or as a "(k/n)" chain of the author's own replies.
async fn write_chain(
    agent: &Agent,
    text: &str,
    tags: &[String],
    reply: Option<(StrongRef, StrongRef)>,
) -> Result<StrongRef> {
    if crate::richtext::graphemes(text) > MAX_BODY_GRAPHEMES {
        return Err(SocialError::Invalid(format!(
            "That's too long; posts can be up to {MAX_BODY_GRAPHEMES} characters."
        )));
    }
    let parts = number_parts(split(text, MAX_GRAPHEMES));
    let mut first: Option<StrongRef> = None;
    let (mut root, mut parent) = match reply {
        Some((root, parent)) => (Some(root), Some(parent)),
        None => (None, None),
    };
    for part in parts {
        let mut record = json!({
            "$type": "app.bsky.feed.post",
            "text": part,
            "tags": tags,
            "createdAt": now_rfc3339(),
        });
        let facets = link_facets(&part);
        if !facets.is_empty() {
            record["facets"] = Value::from(facets);
        }
        if let (Some(root), Some(parent)) = (&root, &parent) {
            record["reply"] = json!({
                "root": { "uri": root.uri, "cid": root.cid },
                "parent": { "uri": parent.uri, "cid": parent.cid },
            });
        }
        let created: CreatedRecord = agent
            .post(
                "com.atproto.repo.createRecord",
                &json!({
                    "repo": agent.did(),
                    "collection": "app.bsky.feed.post",
                    "record": record,
                }),
                None,
            )
            .await?;
        let this = StrongRef {
            uri: created.uri,
            cid: created.cid,
        };
        if root.is_none() {
            root = Some(this.clone());
        }
        parent = Some(this.clone());
        first.get_or_insert(this);
    }
    first.ok_or_else(|| SocialError::Invalid("Nothing to post.".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_fit_bluesky_limits() {
        assert_eq!(
            port_tag("openrct2-openrct2"),
            "portsideapp-openrct2-openrct2"
        );
        let long = "a".repeat(80);
        let tag = port_tag(&long);
        assert_eq!(tag.len(), MAX_TAG_LEN);
        assert!(tag.starts_with(&format!("portsideapp-{}", "a".repeat(43))));
        assert_eq!(port_tag(&long), tag);
        let q = BoardQuery {
            port_id: Some("x".into()),
            sub_forum: Some(SubForum::Bugs),
            ..BoardQuery::default()
        };
        assert_eq!(q.tags(), vec!["portsideapp", "portsideapp-x", "psapp-bugs"]);
        let site = BoardQuery {
            site_board: Some(SiteBoard::Features),
            ..BoardQuery::default()
        };
        assert_eq!(
            site.tags(),
            vec!["portsideapp", "portsideapp-site-features"]
        );
        assert_eq!(port_from_tags(&site.tags()), None);
        assert_eq!(
            SiteBoard::from_tags(&site.tags()),
            Some(SiteBoard::Features)
        );
    }

    #[test]
    fn titles() {
        assert_eq!(
            title_and_body("Crash on start\n\nIt closes."),
            ("Crash on start".to_owned(), "It closes.".to_owned())
        );
        let (t, b) = title_and_body(&"word ".repeat(40));
        assert!(t.ends_with('…'));
        assert!(b.starts_with("word"));
    }
}
