# Spec: Friends, chat and game invites on Bluesky (phase 1)

Status: Draft · Decision: ADR-0021 (amends ADR-0016) · Consumers: desktop app (Friends,
chat, game page), website (invite links land on its catalog page). Code:
`client/crates/portside-social` (`graph`, `chat`, `invite`).

## Friends
- A **friend** is an account you follow on Bluesky that follows you back (`getFollows` for
  the signed-in account, kept when `viewer.followedBy` is set). Up to 3,000 follows are read.
- **Add friend** writes an `app.bsky.graph.follow` record to your own account. The person is
  a friend once they follow back. **Unfollow** deletes that record.
- **Follows you**: your followers (first 300) you don't follow back; "Follow back" makes them
  friends.
- **Find people**: `app.bsky.actor.searchActorsTypeahead`, 10 results.
- Blocked (either way) and muted accounts are left out of every list.

## Chat
- Bluesky direct messages: `chat.bsky.convo.*`, called on the user's account host with the
  header `atproto-proxy: did:web:api.bsky.chat#bsky_chat`. Needs the `transition:chat.bsky`
  scope; older sign-ins without it see "Sign out and in again".
- Opening a chat with someone: `getConvoForMembers` with their DID. Their settings may refuse
  it (`NotFollowedBySender`, `MessagesDisabled`, blocks); the app shows a plain message
  (`E_CHAT_NOT_ALLOWED`).
- Messages: `getMessages` (50 per page, shown oldest first), `sendMessage` with link facets
  for every `http(s)://` link in the text, up to 1,000 characters. `updateRead` when a chat
  is on screen. Message requests can be accepted (`acceptConvo`); replying accepts too.
- Updates: the app polls `getLog` from the newest conversation `rev` (every 4 seconds while
  a chat is on screen, every 45 seconds otherwise) and sends `chat-changed` events:
  `message`, `deleted`, or `changed` (reload the list). System messages of group chats are
  skipped.

## Game invites
An invite is an ordinary direct message:

```
🎮 Want to play {title}? Get it on Portside: {site}/catalog/?q={title}&port={port id}
```

- `{site}` is where the website and catalog are published (the app's catalog address,
  `https://erickbarrera.github.io/portside-catalog` by default). The link carries a link
  facet, so every Bluesky app shows it as a link; the website opens its catalog searched for
  the title.
- Portside recognises a message as an invite when a link (from a facet or the text) has the
  same origin and path as `{site}/catalog/` and a `port` query value made of letters, digits,
  `-`, `_` and `.` (not starting with `.`). It then shows an invite card: the game, Play when
  installed, otherwise Get the game (its page in the app).
- Sending: Invite a friend on the game page (lists friends), or "Invite to play" in a chat
  (lists installed games).
- Not yet: joining a friend's lobby. Manifests already reserve `online.join_uri`
  (manifest-format.md); a later version can add `&join=` to the link.

## Not in phase 1
Presence ("online", "playing now"), Portside-run group chats, voice, push notifications.
They need a server (ADR-0016, phase 2).

## Changelog
- 2026-10-14 — Draft (ADR-0021).
