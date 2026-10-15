# Spec: AT Protocol lexicons

Namespace `gg.portside.*` is a placeholder; NSIDs require control of the domain
`portside.gg` (DNS TXT `_lexicon` for publication). **Do not publish records to the live
network until the final domain is set** — use a local dev PDS.

Source of truth: JSON lexicon files in `lexicons/gg/portside/**.json` [TBD: not created
yet; the backend that would have held them is retired, ADR-0015]. Codegen with
`@atproto/lex-cli` (TS types + validators) for `client/ui` and the website. Rust types for the client core are
generated or hand-written in `client/crates/portside-atproto` [TBD: pick codegen].

All records: `createdAt` (datetime) required; `key: tid` unless noted.

| NSID | Key | Fields (summary) |
|---|---|---|
| `gg.portside.actor.profile` | `literal:self` | `bio` (≤300 graphemes), `favoriteGames` [catalog ids ≤6], `platforms` [windows,macos,linux,android,steamdeck], `banner` blob |
| `gg.portside.library.entry` | `tid` | `game` (portId), `state` (playing/beat/backlog/dropped), `rating` 1–10?, `hoursBucket` (0,1,5,10,25,50,100+), `visibility` must be public (private entries stay on the device or in the user's storage) |
| `gg.portside.feed.review` | `tid` | `game`, `portVersion`, `os`, `recommend` bool, `text` (≤5000 graphemes), `facets`, `langs` |
| `gg.portside.feed.collection` | `tid` | `name`, `description`, `items` [{game, note}] ≤200 |
| `gg.portside.feed.report` | `tid` | compat report: `game`, `portVersion`, `os`, `arch`, `device` (free text ≤64), `result` (works/minor_issues/major_issues/broken), `notes` |
| `gg.portside.graph.follow` | `tid` | `subject` (did). Client offers import from `app.bsky.graph.follow` |
| `gg.portside.forum.thread` | `tid` | `board` (board id string), `title` (≤300), `text` (≤20000), `facets`, `tags` [≤5], `spoiler` bool, `bugMeta` {portVersion, os} optional |
| `gg.portside.forum.reply` | `tid` | `root` strongRef (thread), `parent` strongRef (thread or reply), `text`, `facets` |
| `gg.portside.forum.reaction` | `tid` | `subject` strongRef, `emoji` from allowed set |

Before finalizing forum lexicons, evaluate `forum.barazo.topic.post`,
`forum.barazo.topic.reply`, `forum.barazo.interaction.reaction` (MIT, alpha) and adopt
them if they map cleanly (ADR-0007).

## OAuth scopes
Phase 1 (ADR-0021, ADR-0022) requests `atproto transition:generic transition:chat.bsky`:
posting and following (boards, friends) and Bluesky direct messages. Later, request a
permission set covering only the collections used (`app.bsky.feed.post`,
`app.bsky.graph.follow`, `repo:gg.portside.*`) plus `rpc:chat.bsky.*` for the chat service.
[TBD: switch once granular permissions show well on Bluesky's consent page.]

## Changelog
- 2026-10-14 — Phase 1 scopes (ADR-0021, ADR-0022).
- 2026-10-14 — Lexicon files no longer live in `backend/` (retired, ADR-0015).
- 2026-10-13 — draft.
