# Save sync — client implementation

Protocol: `../../docs/specs/save-sync-protocol.md` (v2). Decision: ADR-0023 (a synced folder
first, plain files, no recovery phrase). This page covers the client side.

## Pieces
- `crates/portside-sync` (no Tauri):
  - `storage`: the `Storage` trait (read, write whole file, exists, list) and
    `FolderStorage`, which writes through a temporary name and rename, skips `*.tmp`
    files, and reports iCloud's `.name.icloud` placeholders as "not downloaded yet".
  - `vault`: `vault.json`, device records and snapshots (JSON), blobs (the save files
    unchanged, named by BLAKE3 hash).
  - `scan`: finds save files under a root with include/exclude globs (NFC paths, no
    symlinks); `restore` copies the current saves to the trash folder, writes each file via
    `*.portside-tmp` + rename with the snapshot's mtime, and removes files that aren't in
    the snapshot.
  - `engine::Syncer`: `sync` (upload if changed, then restore or report a conflict) and
    `keep` (settle a conflict with a merge snapshot).
  - `state::LocalState`: `{app_data}/saves.db`.
- `src-tauri/src/saves.rs`: opens the vault in the folder saved in `saves.db`, works out each game's save folder,
  runs syncs off the async runtime (one per game at a time), emits `saves-changed`.
  `game_launch` syncs first; a conflict fails the launch with `E_SAVES_CONFLICT`; any
  other problem lets the game start. A sync also runs after the game exits.
- UI: `lib/saves.svelte.ts` (store), `SavesSetup.svelte` (Settings → Cloud saves),
  `CloudSaves.svelte` (game page).

## Where a game's saves are
1. A folder the player chose for the game on this device (everything in it syncs).
2. Else the manifest's `saves.root` for this OS, placeholders resolved by
   `portside_core::paths::resolve_placeholders` (`{install}` is the current install
   folder), filtered by `saves.include` / `saves.exclude`. The `saves` section is fetched
   with the port page and kept in `saves.db` so sync works offline.
3. Else the game page offers "Choose save folder".

## Setting up
- Pick a folder (synced folders found on this computer are listed:
  `portside_core::paths::synced_folders`) and that's it (`saves_use_folder`). A folder
  without a vault gets one in `<folder>/Portside Saves/`; a folder that has one (it shows
  "Has Portside saves") joins it. No phrase or key.
- Turning off forgets the folder on this device; the folder and the saves on disk are left
  alone.

## UI states (game page)
`off`, `no_folder`, `not_synced`, `syncing`, `synced`, `conflict`, `unavailable`, `error`.
The conflict picker shows each side's device, time, file count, size and game version;
keeping one restores it if needed and records a merge snapshot so every device follows.
When a conflict stopped a launch, "Play without syncing" starts the game anyway (it syncs
again on exit, which keeps both sides in the history).

## Not built yet
Retention (old snapshots and blobs), more storage adapters (WebDAV, S3, provider APIs),
files over 256 MB, "another device is playing" warning, opt-in encryption, Android, a
history view to restore an older snapshot, the system tray keeping the
app alive for post-exit sync.
