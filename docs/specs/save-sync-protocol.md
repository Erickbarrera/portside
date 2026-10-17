# Spec: Save sync protocol (v2)

Status: Draft · Decisions: ADR-0005 (user-owned storage), ADR-0015 (no Portside server),
ADR-0023 (storage folder first, history instead of a head pointer, no encryption) ·
Consumer: desktop app (`client/crates/portside-sync`).

## The idea in one paragraph
Each player keeps a **save vault** of plain files in storage they already have. The first kind
of storage is a folder that their own cloud app keeps in sync (iCloud Drive, Dropbox,
OneDrive, Google Drive for desktop, Syncthing, a network share). Portside never runs a
server. Every sync writes new, uniquely named files and never edits a
file another device may be writing, so it works on storage with no locking or
compare-and-swap. Devices agree on the latest save by reading the history: each snapshot
names its parents, and the snapshots nobody builds on are the current ones. One current
snapshot means everyone agrees; two or more is a conflict the player settles once.

## Concepts
- **Save root** per (port, OS): manifest `saves.root` with placeholders resolved
  (manifest-format.md, "Path placeholders"), filtered by `saves.include` / `saves.exclude`
  globs. The player can set their own folder for a game; it wins over the manifest.
  `{saf:…}` and `{flatpak:…}` roots aren't supported yet.
- **Vault**: the folder `Portside Saves/` inside the chosen storage location.
- **Snapshot**: the full list of a game's save files at one moment. Immutable.
- **Base**: per device and game, the snapshot this device last matched (stored locally).
- **Current snapshots** ("leaves"): snapshots no other snapshot lists as a parent.

## Vault layout
```
Portside Saves/
  vault.json                              written once when the vault is created
  devices/<device id>.json                device record
  playtime/<device id>.json               play time on that device
  games/<port id>/<snapshot id>.<device id>.json   snapshot
  blobs/<id[0..2]>/<id>                   a save file's bytes, unchanged
```
Everything is plain: the save files are stored as they are and every record is JSON, so a
player can open the folder, back it up, or copy a save back by hand. There is no
encryption and no recovery phrase (ADR-0023); the folder is as private as the player's
cloud account.
- `vault.json`: `{"format": "portside-saves", "version": 2, "vault_id": "<ULID>"}`. A device
  that finds it uses that vault; a folder without it gets a new one.
- Device ids and snapshot ids are ULIDs. A snapshot file is named
  `<snapshot ULID>.<device id>.json`, so two devices never write the same name.
- `<port id>` is the catalog id, which only uses `a-z`, `0-9`, `-`, `_` and `.` and never
  starts with `.`; anything else is refused so a record can't point outside the vault.
- **Blob id** = hex(BLAKE3(file bytes)), 64 digits. The same file is stored once however
  many snapshots or devices have it. A blob is checked against its id when read.
- Files are stored whole, up to 256 MiB each; larger files make the game unsyncable
  [TBD: chunking]. Records over 16 MiB are treated as damaged.
- Files whose names end in `.tmp` are partial writes: ignore them.

## Snapshot
```json
{
  "id": "01JABC…", "port_id": "openrct2-openrct2",
  "device_id": "01JDEV…", "device_name": "Erick's MacBook", "os": "macos",
  "created_at": "2026-10-14T22:00:00Z", "parents": ["01J…"],
  "port_version": "v0.5.6",
  "files": {"park1.park": {"size": 123456, "mtime": 1792000000, "blob": "<id>"}}
}
```
`files` keys are paths relative to the save root, `/`-separated, NFC. `mtime` is restored
when the file is written back. A snapshot with no files is valid (the player deleted every
save). A snapshot whose `port_id` or file name doesn't match where it was found is ignored.

## Device record
`{"id", "name", "os", "app_version", "last_sync": "<RFC 3339>"}`, rewritten on each sync.

## Play time
Each device keeps its own play time (from its local play sessions) in
`playtime/<device id>.json`, and only that device writes the file, so it can simply be
rewritten:
```json
{
  "device_id": "01JDEV…", "device_name": "Erick's MacBook",
  "updated_at": "2026-10-17T22:00:00Z",
  "games": {"openrct2-openrct2": {"total_secs": 12600, "last_played": 1792000000, "sessions": 4}}
}
```
`last_played` is Unix seconds when the last session ended. A device rewrites its file after
a game exits and when cloud saves are turned on, then reads every other device's file. A
game's play time on screen is this device's own sessions plus every other device's
`total_secs` and `sessions`; last played is the latest of them. A file whose `device_id`
doesn't match its name, or that doesn't open, is skipped; so are game keys that aren't port
ids. Play time syncs for every game played, whether or not Portside knows its save folder.

## Sync (one game)
Runs before the game starts, after it exits, and when the player presses "Sync now". One
sync per game at a time.
1. Scan the save root: `{path → blob id}`. A missing root counts as empty.
2. If the scan differs from the base's files: upload any blobs not in the vault yet, then
   write a snapshot with `parents = [base]` (or `[]` when there is no base). It becomes the
   new base. Nothing in the vault is ever overwritten.
3. Read the game's snapshots and find the current ones. Snapshots whose parents are missing
   (not synced yet) still count as parents.
4. One current snapshot: if it isn't the base, restore it (the save folder matches the base
   at this point, so nothing local is lost) and make it the base.
5. Several current snapshots with the same files: treat the newest as the one, as in 4.
6. Several with different files: **conflict**. The game page shows each side (device, time,
   file count, size) and the player keeps one. Keeping a side writes a snapshot with that
   side's files and `parents` = every current snapshot, restores it if needed, and makes it
   the base. The other side stays in the history.

Before the game starts, a conflict blocks the launch until it's settled (or the player
plays anyway, which syncs again after exit). A sync that can't reach the vault (folder
missing, disk full) lets the game start and shows an error.

## Restore
- Each file is written to `<name>.portside-tmp`, flushed, then renamed over the old one;
  `mtime` is set from the snapshot.
- Before anything is overwritten or removed, the current save folder is copied to
  `{app_data}/saves-trash/<port id>/<UTC time>/`; copies older than 30 days are removed.
- Files not in the snapshot (and matched by the include/exclude rules) are removed.
- Every blob is checked against its id; a mismatch stops the restore before anything is
  written.

## Not in v2 yet
- Deleting old snapshots and blobs [TBD: retention, e.g. last 20 + one a day for 30 days].
- Warning when another device is playing the same game right now.
- Storage adapters other than a folder: WebDAV, S3, Google Drive / Dropbox / OneDrive
  direct [TBD: register OAuth apps]. They keep this layout and need only list, read,
  write (whole file) and delete.
- Opt-in encryption for players who want the folder private from their cloud provider
  [TBD: only if players ask; ADR-0023].
- Android (Storage Access Framework roots).

## Changelog
- 2026-10-17 — v2: play time per device in `playtime/<device id>.json`, added up across
  devices.
- 2026-10-15 — v2 revised before release (ADR-0023): no encryption and no recovery phrase.
  Save files are stored unchanged, records are plain JSON, games are folders named by port
  id, blob ids are plain BLAKE3. `vault.json` lost `kdf` and `check`.
- 2026-10-14 — v2: no server. The vault lives in the player's storage (a synced folder
  first); history with parents replaces the head pointer and compare-and-swap; snapshot
  lists and paths are encrypted too; subkeys from BLAKE3 `derive_key`; restore copies the
  old saves to a trash folder first.
- 2026-10-14 — Server-side parts superseded by ADR-0015; v2 pending.
- 2026-10-13 — v1 draft.
