# Install, launch, update

## Install
Built in `portside-installer` (ADR-0014); `src-tauri/src/installs.rs` runs it in the
background and reports `install-changed` events.
1. The game page asks for a plan (`install_plan`): Rust fetches `v1/ports/{id}.json` and reads its
   manifest. Unknown `schema` → "needs a newer Portside".
2. Pick build: exact OS+arch → `universal` → x86_64 on arm64 macOS or Windows (Rosetta or
   emulation; the plan says so) → none ("not available for your platform").
3. Resolve the release: a non-revoked release from the catalog with a file matching the
   build's `asset` glob; else the GitHub REST API (`/releases/latest`, or the release list for
   `prerelease` and `tag_pattern`). The glob must match exactly one file. A `url_list`
   manifest (ADR-0024) lists its one version's files itself, so without a catalog release the
   plan is made from the manifest, with no network.
4. Checksum: catalog `sha256` → GitHub `digest` → a checksum file in the release.
   `checksum: upstream` without one fails; otherwise the plan says it can't be verified.
   `url_list` files always carry a `sha256`; the plan fails without one.
5. The user confirms. An unsigned macOS build (`unsigned: true`) shows a warning and needs
   "Install anyway". On a Steam Deck with an SD card in, a new install also asks where it
   goes (`install_storage`; platforms.md, "SD card"). `{library}` below is that place's library
   folder; an update stays in the folder the game is in.
6. Download to `{library}/.partial/` with Range resume, from GitHub hosts only (checked on
   every redirect); a `url_list` port's files only from its manifest's `source.hosts`. Cancel keeps the partial file, so the next try resumes.
7. Verify SHA-256, unpack into `{library}/.staging/`, apply `strip_root`; check the launch
   target (`exe` or `app_bundle`) exists; `chmod_x`; clear `com.apple.quarantine` on an
   unsigned bundle the user agreed to. Then move it to `{library}/{id}/{version}/`, write
   `{library}/{id}/current`, and record it in `{app_data}/library.db`.
8. If `assets_required`: import wizard [TBD: step after launch]; launch stays blocked until
   satisfied.

Archives: `zip`, `tar.gz`, `tar.xz`, `7z`, `dmg` (macOS: `hdiutil` + `ditto`), `appimage` and
`none` (saved under `launch.exe`, so a versioned AppImage name keeps one launch path). A
build with `nested` unpacks the asset into `.staging/outer/`, finds the one file matching
`nested.file`, unpacks that instead and drops the rest (manifest-format.md, "Archives").
Not yet: `apk`.
Entries with absolute paths or `..`, and writes through a symlink the archive created, fail
the install.
Uninstall deletes `{library}/{id}/` and its records. A game on an SD card that isn't in is
forgotten, and its files stay on the card.

## Launch
Built in `portside-launcher` (ADR-0017); `src-tauri/src/games.rs` runs it and reports
`game-changed` events.
1. Pre-launch sync (save-sync.md). Show conflict UI if needed. [TBD: with save sync]
2. Refuse while the game is installing or already running. A revoked version blocks launch
   and offers rollback [TBD: with updates]. `assets_required` isn't checked yet; ports that
   need a ROM ask for it themselves [TBD: import wizard].
3. Work out the command from the stored manifest build (works offline): `exe` relative to
   the install folder, or for a macOS `app_bundle` the bundle's
   `Contents/MacOS/<CFBundleExecutable>`. `args`, `cwd` (default: the install folder) and
   `env` come from the manifest, with `{install}` replaced by the install folder. Paths that
   leave the install folder are refused.
4. macOS unsigned build: consent was given at install (ADR-0014) and the bundle's
   quarantine flag is already cleared; nothing to ask here. An Intel-only build on Apple
   silicon without Rosetta fails with `E_NEEDS_ROSETTA` and says how to install it.
5. Spawn in its own process group (macOS, Linux), output to
   `{app_data}/logs/{port_id}.log` (replaced each run). Record a play session in
   `library.db` and move its end forward every 60 seconds.
6. Stop: `SIGTERM` to the group, `SIGKILL` after 5 seconds (Windows: `taskkill /T`, then
   `/F`). The game counts as ended when its main process exits [TBD: process tree for
   ports that relaunch themselves].
7. On exit: save the session end, send `exited` with the exit code and new play time. A
   non-zero code the user didn't cause shows "quit with error code N" and a link to the log.
   Post-exit sync [TBD].

## Update
- Check: on app start + every 6h, compare installed version vs latest from catalog delta.
- Per-port policy: auto | notify (default) | pinned.
- Revoked versions (`revoked: true` from API) block launch and offer rollback.

## Uninstall
Refused while the game runs. Remove install dirs; play time is kept; keep saves by default (ask). Never delete files outside `{library}`
except when the user chooses "delete local saves" for that port.
