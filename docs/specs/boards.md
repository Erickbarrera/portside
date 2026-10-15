# Spec: Discussion boards on Bluesky (phase 1)

Status: Draft · Decision: ADR-0016 (phase 1 social on Bluesky; replaces ADR-0007 until a
phase 2 backend) · Consumers: desktop app (Community tab), catalog (`v1/boards.json`),
website (optional read-only pages). Built in Milestone 3, after sign-in.

## The idea in one paragraph
Every board thread is an ordinary Bluesky post carrying **hidden tags**. A Bluesky post can
carry up to 8 tags in its `tags` field that don't show in the text, and Bluesky's search can
filter on them (`app.bsky.feed.searchPosts`, `tag=` with AND matching). Portside writes
threads and replies with tags like `portsideapp`, `portsideapp-openrct2-openrct2` and
`psapp-help`. Plain `#portside` is used on Bluesky for plenty of unrelated things, so every
tag starts with the more distinctive `portsideapp` or `psapp-`.
It reads a board by searching for those tags and reads a thread with Bluesky's normal thread
API. The posts and accounts live on Bluesky, and pins, locks and moderation rules live in
the static catalog. Nothing runs on a Portside server.

Checked against the current lexicons (2026-10-14): `app.bsky.feed.post` has `tags`
(max 8, 64 chars each) and 300-character text; `searchPosts` takes `tag[]`, `sort`
(`latest`/`top`), `author`, `since`; `threadgate` can stop replies and hide replies.

## Works across AT Protocol, not only Bluesky
Erick's requirement (2026-10-14): boards must work with other AT Protocol sites, not just
bsky.app.
- **Any account host.** Sign-in is AT Protocol OAuth against the user's own PDS:
  bsky.social, a self-hosted PDS, Blacksky, Eurosky or any other. Posts are written to that
  account, not to Bluesky's servers.
- **Any Bluesky-compatible app shows the threads.** Threads use the network's shared post
  record (`app.bsky.feed.post`). Bluesky, Blacksky, Deer.social, Graysky and other clients
  all show them, and replies from any of those apps appear in the Portside thread.
- **Pluggable index.** The app doesn't hard-code Bluesky's index. It asks "an index"
  (an AppView) for searches and threads, reached through the user's own PDS. The default is
  whatever the user's PDS uses (Bluesky's for most people); Settings can pick another, such
  as Blacksky's. Signed out, a configurable default is used.
- **Other record types, later.** Forum-specific AT apps use their own records, for example
  Barazo (`forum.barazo.*`). The design keeps a "board source" interface so a second adapter
  could read Barazo threads into the same board, and optionally cross-post. Long-form sites
  (Leaflet, WhiteWind) and link sites (Frontpage) show as link cards when a post links to
  them. Not in the first version.

## How each Steam feature maps

| Steam boards | On Bluesky | Notes |
|---|---|---|
| A board per game | Tag `portsideapp-<port id>` on every post in it | Board list comes from the catalog (every port gets a board) |
| Sub-forums (General, Help, Bugs, Mods, Multiplayer) | A second tag: `psapp-general`, `psapp-help`, `psapp-bugs`, `psapp-mods`, `psapp-mp` | Search with both tags (AND) |
| Site-wide boards (Announcements, Portside help, Off-topic) | Tags `portsideapp-site-<name>` | Same mechanics |
| Thread title | First line of the post, shown bold; the composer has a Title field and writes "Title\n\nBody" | Bluesky shows it as a normal post |
| Long first post | Split into a chain of the author's own replies ("1/3"), stitched back together by the app | 300-character limit per post |
| Replies, quoting, images | Normal Bluesky replies, quote posts, image embeds | `getPostThread` returns the whole tree |
| Sort by latest activity ("bump") | Replies written from Portside carry the board tags too. The app searches the board's tag (latest first) and groups results by thread root, so a new reply bumps its thread | Replies made in the Bluesky app still show inside the thread but don't bump it |
| Most popular | `sort=top` | |
| Reply count, likes | From the post view (`replyCount`, `likeCount`) | |
| Search in a board | `searchPosts` with words + the board tag | |
| Pinned / sticky threads | `v1/boards.json` in the static catalog: pinned post links per board | Edited by PR in the private repo, live within an hour |
| Lock a thread | Thread author: Bluesky threadgate (nobody can reply). Moderators: listed as locked in `boards.json`; the app hides the reply box and shows nothing posted after the lock | Bluesky can't stop replies from other apps; the app filters them out |
| Delete / hide a thread or reply | `boards.json` hidden list; authors hide replies to their own threads with threadgate | Hidden in Portside, still on Bluesky |
| Ban a user | A Bluesky moderation list owned by the Portside account. The app hides everyone on it | Moderators edit the list in the normal Bluesky app, effective immediately |
| Report | Bluesky's report (goes to Bluesky moderation) + "Report to Portside" (opens a GitHub issue form) | |
| Unread markers, followed threads | Kept on the device (SQLite) | Bluesky's own notifications cover replies to your posts and mentions |
| Moderator / developer badges | `boards.json` lists moderator accounts; port maintainers can be listed per board | |

## What a post looks like
```json
{
  "$type": "app.bsky.feed.post",
  "text": "Crash on launch after 0.5.6 on macOS\n\nSince updating, the app closes right away…",
  "tags": ["portsideapp", "portsideapp-openrct2-openrct2", "psapp-bugs"],
  "langs": ["en"],
  "createdAt": "2026-10-14T15:00:00Z"
}
```
Port ids longer than 52 characters get a shortened tag: `portsideapp-`, the first 43
characters of the id, `-`, and the first 8 hex digits of the id's SHA-256 (64 characters in
all, Bluesky's tag limit). Every client builds the same tag from the id.

Rules the app follows (built 2026-10-14, `client/crates/portside-social/src/boards.rs`):
- **Title**: the first line of the first post, at most 120 characters; the composer writes
  "Title\n\nBody". A first post without a short first line gets its first 80 characters as
  the title.
- **Long posts**: text over 300 graphemes is split at paragraph, line or word breaks into a
  chain of the author's own replies, each ending in " (k/n)". The app stitches the chain back
  into one post when it shows the thread. Up to 2,800 characters per post or reply.
- **Tags on every post and reply written from Portside**: `portsideapp`, the port tag and
  the sub-forum tag. Replies copy the thread's `portsideapp…`/`psapp-…` tags (at most 8).
- **Reading a board**: `searchPosts` with `q` set to the board's tag as a hashtag
  (`#portsideapp-<id>`, `#portsideapp-site-<name>` or `#portsideapp`, plus any search words;
  `q` is required), every board tag as `tag`, `sort` and `limit=100`. Only posts that carry
  `portsideapp` as a hidden tag count: someone typing the hashtag in a post's text doesn't put
  it on a board. Results are grouped by thread root;
  roots that weren't in the results are fetched with `getPosts`. [TBD: confirm on the live
  network that a hashtag query matches the hidden `tags`.]
- **Reading a thread**: `getPostThread` with `depth=50`; replies shown oldest first, with
  "Replying to …" when a reply answers another reply. Blocked and muted authors are left out.
- **Portside's own boards** (Erick, 2026-10-14): `portsideapp-site-help` (help with
  Portside) and `portsideapp-site-features` (feature requests), each with the `portsideapp`
  tag. No
  sub-forums. Threads start from the Community tab.
- **Community tab**: "All discussions" (the `portsideapp` tag alone, across every game and
  Portside's boards), "Portside help" and "Feature requests".

## Reading without signing in
Signed-in users read through their own account. Whether Bluesky's public endpoint still
allows anonymous search could not be checked from here (the sandbox can't reach it). If it
doesn't, signed-out users see a "Sign in to read the boards" prompt and pinned threads only.
This is the first thing to test.

## Trade-offs to accept
- Board threads appear in the author's followers' Bluesky timelines like any other post.
  That's good for reach, but a Bluesky post can't be made "boards only".
- Anyone can add the tags from any app, so spam with the right tags will appear. The ban
  list, the hidden list and Bluesky's own labels handle it after the fact, not before.
- Bluesky's search is the index. If it changes or starts limiting tag search, the boards stop
  updating until we adapt. That's a dependency on a large, stable service we don't control.
- Search pagination isn't guaranteed to reach very old threads; boards show roughly the
  most recent few hundred threads, plus anything pinned.
- Moderator locks and hides only apply inside Portside.

## Alternatives considered
- **Our own record types + our own index (ADR-0007):** a true forum, but needs a server.
- **Barazo** (an AT Protocol forum with its own index): their service, still alpha, and
  boards would live on their site's terms.
- **A Bluesky custom feed per board:** needs a feed server.
- **Plain hashtags in the text:** works, but clutters posts and anyone sees `#portsideapp…`
  noise. Hidden tags do the same job invisibly.

## Pieces to build (Milestone 3, after sign-in)
1. Finish this spec: exact tag rules, title convention, the `v1/boards.json` shape (and add
   it to static-catalog.md).
2. Catalog: `manifests/boards.yaml` → `v1/boards.json` (sub-forums, pins, locks, hidden
   posts, moderators, ban-list link).
3. App: a `portside-boards` crate (search, thread stitching, bump ordering, filters) and a
   Community tab: board list → board (sub-forum tabs, threads) → thread → composer.
4. Website (optional): read-only board pages, if anonymous search works.

## Changelog
- 2026-10-14 — Tags renamed to `portsideapp…` and `psapp-…` (Erick: `#portside` is
  widely used for other things); boards ignore posts without the hidden `portsideapp` tag.
- 2026-10-14 — Portside's own boards: help and feature requests.
- 2026-10-14 — Exact tag shortening, title and long-post rules, how boards and threads are
  read (first version in the app). `v1/boards.json` (pins, locks, hidden posts) still to come.
- 2026-10-14 — Draft (ADR-0016).
