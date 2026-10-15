# Accounts & social — client

Phase 1 runs on Bluesky with no Portside server (ADR-0016). Built 2026-10-14: sign-in,
friends, chat, game invites and discussion boards. Specs: `../../docs/specs/boards.md`,
`../../docs/specs/friends-chat.md`. Crates: `portside-atproto` (sign-in and authenticated
calls) and `portside-social` (friends, chat, invites, boards); Tauri side in
`src-tauri/src/social.rs` and `src-tauri/src/commands/social.rs`.

## Sign-in (ADR-0022)
- `/signin`: "Sign in with Bluesky" (starts at bsky.social), or a handle on any AT Protocol
  host, or "Continue without signing in".
- Flow: handle → DID (`https://<handle>/.well-known/atproto-did`, else
  `com.atproto.identity.resolveHandle` on Bluesky's public index) → DID document
  (plc.directory or did:web) → account host (PDS) → its sign-in server
  (`/.well-known/oauth-protected-resource`, then `/.well-known/oauth-authorization-server`)
  → PAR with PKCE and a DPoP proof → system browser → `http://127.0.0.1:<port>/callback` →
  token request with DPoP → check the account (`sub`) belongs to that sign-in server.
- Client: the atproto "localhost client" (`client_id` =
  `http://localhost?redirect_uri=http://127.0.0.1/callback&scope=…`), nothing hosted.
  Scope: `atproto transition:generic transition:chat.bsky`.
- The session (tokens and the DPoP private key) is one keychain entry: service
  `gg.portside.desktop`, account `atproto-session`. Restored at start; renewed with the
  refresh token a minute before it runs out, or when the host rejects the token. A dead
  refresh token signs out (`auth-changed` with no account, `E_SIGNED_OUT`).
- Sign out revokes the tokens (best effort) and deletes the keychain entry.
- Android: Custom Tab + app link instead of the loopback listener (later).

## Friends, chat, invites (ADR-0021)
- Friends = mutual follows. Add friend follows; Unfollow deletes the follow. "Follows you"
  lists followers you don't follow back. Find people searches Bluesky.
- Chat = Bluesky direct messages through the user's host (`atproto-proxy`). A background
  task polls the message log (4 s while a chat is on screen via `chat_watch(true)`, 45 s
  otherwise) and emits `chat-changed`. Unread counts show on Friends in the sidebar.
- Invites = a DM with the website link `{site}/catalog/?q=…&port=…`; shown in Portside as
  a card with Play or Get the game. Sent from the game page (Invite a friend) or a chat
  (Invite to play).

## Boards
Community tab (all discussions, plus Portside's own Help and Feature requests boards at
`/community?board=help|features`), a board per game (`/port/:id/board`, sub-forum tabs, search,
latest/top, new thread), threads (`/thread/:uri`, replies, "Open in Bluesky"). Every write
says it is public on Bluesky. Pins, locks, hidden posts and the ban list from
`v1/boards.json` come next.

## Later
Profile, library entries, reviews, collections, compat reports (`gg.portside.*` records,
`../../docs/specs/lexicons.md`) once the NSID domain is set. Presence, group chats, lobby
joins in phase 2.
