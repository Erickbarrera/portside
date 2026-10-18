# Client architecture

## Process model
- **Main process (Rust)**: Tauri runtime + feature crates. Owns SQLite DB, download queue,
  sync engine, game process monitoring.
- **UI (webview)**: Svelte SPA. Talks to Rust only via typed commands and events.
- **Background mode**: app keeps running in the system tray when the window closes
  (configurable) so downloads and post-exit sync finish. Autostart optional
  (`tauri-plugin-autostart`).

## Crates
| Crate | Responsibility | Key deps |
|---|---|---|
| portside-core | Domain types (Port, Build, Manifest, Snapshot), config, `paths` resolver, error types, OS/arch detection | serde, directories, thiserror |
| portside-catalog | SQLite cache of catalog; ETag sync of `v1/snapshot.json`; search (FTS5); local library state | rusqlite + FTS5 |
| portside-installer | Built: pick build, resolve release (API, else GitHub; ADR-0014), resumable download from GitHub hosts (or a `url_list` manifest's own hosts, ADR-0024), SHA-256 verify, unpack (zip, tar.gz, tar.xz, 7z, dmg, AppImage, nested archives) into versioned dirs, `library.db` of installs. Planned: download queue (parallel limit 2), apk, rollback | reqwest, zip, tar, flate2, liblzma, sevenz-rust2, sha2, globset |
| portside-assets | Built (2026-10-18): game files (manifest `game_files`) from the player's own copy: finds copies Steam (`portside-steam::apps`), GOG (Galaxy's database, Heroic, `goggame-<id>.info`) and Epic (launcher manifests, Heroic/Legendary) installed, in the places `paths::store_places` lists; matches the needed files (any case, up to four folders down) and copies them. Planned: ROM import (pick file, byteswap, hash against `accept`) | rusqlite, serde_json |
| portside-launcher | Built (ADR-0017): launch command from the stored build (`exe`, or a macOS bundle's executable from `Info.plist`), spawn in its own process group with env/args/cwd and a log file, wait for exit, Stop (TERM, then KILL). Play time is stored by portside-installer's `library.db`. Planned: process tree. Saves sync before launch and after exit (src-tauri `saves.rs`) | tokio::process, plist |
| portside-sync | Built (ADR-0023, save-sync v2): vault of plain files, `Storage` trait with a synced-folder implementation, save folder scan (include/exclude globs), snapshots with parents, conflict detection, restore with a trash copy, `saves.db`. Planned: retention, more storage adapters, chunking | blake3, ulid, globset, rusqlite |
| portside-atproto | Built (ADR-0022): handle→DID→PDS resolution, OAuth as a localhost client (PAR+PKCE+DPoP, loopback redirect), token renewal, authenticated XRPC with DPoP nonces and `atproto-proxy`; `SessionStore` (keychain impl in src-tauri). Planned: blob upload | reqwest, p256, base64 |
| portside-social | Built (ADR-0016, ADR-0021): friends (mutual follows), Bluesky direct messages and log polling, game invite links, boards on tagged posts (search, thread stitching, posting chains) | portside-atproto, unicode-segmentation |
| portside-steam | Built (2026-10-16): Add to Steam on every OS. Finds Steam (`paths::steam_dirs`) and its last-used account, reads and writes the binary `shortcuts.vdf` (keeping entries it doesn't own, one-time `.before-portside` backup), cuts grid artwork (tall, wide, hero) from one picture, tells whether Steam is running, finds the best installed Proton | crc32fast, image, sysinfo |
| portside-api | Typed client for the static catalog's `v1/` files (docs/specs/static-catalog.md) | reqwest, serde |

## Local data
Built: `{app_data}/library.db` (portside-installer): one row per installed version (the
`Installed` record as JSON, including the manifest build so launching works offline) and
the current version per port, and `play_sessions` (start and end per run; the end moves
forward every minute while the game runs). `{library}/{port_id}/{version}/` plus a `current` file naming
the version folder; `{library}/.partial/` holds resumable downloads.
`{app_data}/logs/{port_id}.log` holds the output of each game's last run.
`{app_data}/saves.db` (portside-sync): this device's id, the cloud saves folder and vault
id, and per game the snapshot it last matched, a folder the player chose, and the
manifest's save rules (kept for offline use). `{app_data}/saves-trash/{port_id}/<time>/`
holds the saves that were there before each restore, for 30 days.
`{app_data}/catalog-<catalog address>.db` (portside-catalog, ADR-0012): port
summaries, per-OS rows, an FTS5 title index, the synced revision and the snapshot's ETag.
Synced on start, every 15 minutes and on request: a conditional GET of `v1/snapshot.json`
(304 when unchanged; otherwise the whole file replaces the copy). All search is local and
matches titles only (word prefixes).

Planned:
- Download queue, settings (one database per concern, like the others).
- `{library}/` is `{app_data}/library` now; user-changeable and per-drive libraries later.
- Keychain entries: `gg.portside.desktop` / `atproto-session` (built: the Bluesky sign-in,
  JSON with tokens and the DPoP key) and `saves-key-<vault id>` (built: the cloud saves key).

## IPC (examples; generated bindings are authoritative)
Implemented: `app_info`; `catalog_search` (local copy); `port_get` (`v1/ports/{id}.json`,
falling back to the saved summary with `offline: true`); `catalog_status`;
`catalog_refresh`; `catalog_summaries` (saved summaries for a list of ids, so the Library
shows covers offline); `install_plan`, `install_start`, `install_storage`, `install_cancel`, `installs_active`,
`library_list`, `library_get`, `uninstall`, `install_reveal`; `game_launch`, `game_stop`,
`games_running`, `play_stats`, `game_reveal_log`; `auth_status`, `auth_sign_in`,
`auth_cancel`, `auth_sign_out`; `friends_list`, `friends_follows_you`, `people_search`,
`friend_add`, `friend_remove`; `chat_conversations`, `chat_open`, `chat_get`,
`chat_messages`, `chat_send`, `chat_mark_read`, `chat_accept`, `chat_watch`; `game_invite`;
`board_threads`, `board_thread`, `board_post`, `board_post_site`, `board_reply`;
`saves_status`, `saves_pick_folder`, `saves_use_folder`, `saves_turn_off`, `saves_devices`, `saves_game`, `saves_sync`,
`saves_keep`, `saves_set_game_folder`, `saves_reveal` (`game_launch` syncs first unless
`skip_save_sync`). Events: `catalog-synced` after
every sync attempt; `install-changed` (downloading/verifying/unpacking, then installed,
failed, cancelled or uninstalled); `game-changed` (running, stopping, then exited with the
exit code and new play time); `auth-changed` (signed in, out or expired); `chat-changed`
(new or deleted messages, other conversation changes); `saves-changed` (a game's cloud saves
are syncing, synced, in conflict or failed). Error codes so far: `E_API_UNREACHABLE`, `E_NOT_FOUND`,
`E_API_STATUS`, `E_API_DECODE`, `E_CATALOG_DB` (the `E_API_*` codes now mean the catalog
host), `E_NO_MANIFEST`, `E_MANIFEST_SCHEMA`, `E_NO_BUILD`, `E_RELEASE_MISMATCH`,
`E_GITHUB_RATE_LIMIT`, `E_GITHUB_STATUS`, `E_DOWNLOAD_HTTP`, `E_HOST_NOT_ALLOWED`,
`E_ASSET_HASH_MISMATCH`, `E_CHECKSUM_MISSING`, `E_UNPACK`, `E_NEEDS_CONSENT`, `E_CANCELLED`,
`E_NOT_INSTALLED`, `E_INSTALL_RUNNING`, `E_LIBRARY`, `E_GAME_RUNNING`, `E_LAUNCH`,
`E_NEEDS_ROSETTA`, `E_SIGNED_OUT`, `E_AUTH`, `E_KEYCHAIN`, `E_SOCIAL_UNREACHABLE`,
`E_SOCIAL`, `E_CHAT_NOT_ALLOWED`, `E_INVALID`, `E_SAVES_OFF`, `E_SAVES_UNAVAILABLE`,
`E_SAVES_CONFLICT`, `E_SAVES`, `E_APP_UPDATE`. Updating Portside itself (`app_update.rs`):
`app_update_status`, `app_update_check`, `app_update_install`, event `AppUpdateChanged`
(docs/build-release.md, "Auto-update"). Catalog address: `PORTSIDE_CATALOG_URL`, default
`https://erickbarrera.github.io/portside` (or `PORTSIDE_DEFAULT_CATALOG_URL` at build
time) [TBD: custom domain].
Planned commands: `asset_import`, `update_check`, `rollback`,
`record_put`, `settings_get/set`.

## Error handling
Every user-visible failure maps to an error code (`E_DOWNLOAD_HTTP`, `E_ASSET_HASH_MISMATCH`,
`E_LAUNCH_EXEC`, `E_SYNC_CONFLICT`, ...) with a doc'd message; codes also go to opt-in
telemetry.
