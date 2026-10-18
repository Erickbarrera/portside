# Spec: Port manifest format (v1)

Status: Draft · Owner: specs · Consumers: `catalog/` (validates, publishes in the static
catalog), client installer/launcher/sync (executes)

## Where manifests live
`manifests/ports/<upstream-id>.yaml` in this monorepo (ADR-0011), one file per port, where
`<upstream-id>` matches the awesome-decomp-games `id` (e.g. `harbourmasters-shipwright`).
Contributions come by PR; the catalog workflow validates every file with `catalog/`
(JSON Schema: `catalog/schemas/manifest.v1.json`). The catalog build reads the folder and
publishes each manifest in its port's file (static-catalog.md). The folder stays in the
private repository with the catalog builder (ADR-0015); outside contributors suggest
manifests through issues on the public `portside` repository.

## Schema

```yaml
schema: 1                          # required, integer
id: harbourmasters-shipwright      # required, = upstream id, [a-z0-9-]+
game_key: thelegendofzeldaocarinaoftime   # required, groups ports of one game
display_name: "Ship of Harkinian"  # optional override of upstream title
status: playable                   # required: perfect|playable|in_progress|boots|not_playable
status_notes: "Randomizer requires ..."   # optional, markdown, short
source:
  type: github_releases            # github_releases | url_list | github_actions_artifact (later)
  repo: HarbourMasters/Shipwright  # required for github_*
  channel: stable                  # stable (latest non-prerelease) | prerelease | tag_pattern
  tag_pattern: null                # regex when channel=tag_pattern
  min_version: "9.0.0"             # optional; older releases hidden
builds:                            # required, 1+
  - os: windows                    # windows|macos|linux|android
    arch: x86_64                   # x86_64|arm64|universal
    asset: "SoH-*-Win64.zip"       # glob matched against release asset names, exactly 1 match
    archive: zip                   # zip|tar.gz|tar.xz|7z|none|apk|dmg|appimage
    strip_root: true               # unwrap a single top-level dir
    launch:
      exe: "soh.exe"               # relative to install dir
      args: []
      cwd: "."
      env: {}
    checksum: upstream             # upstream (sha256sums asset) | attestation | none
  - os: linux
    arch: x86_64
    asset: "SoH-*.appimage"
    archive: appimage
    launch: { exe: "soh.appimage" } # appimage/none: the downloaded file is saved under this name
  - os: macos
    arch: universal
    asset: "SoH-*-Mac.zip"
    archive: zip
    nested:                        # optional: the asset holds another archive to unpack
      file: "*.dmg"                # glob matched against paths inside the asset, exactly 1 match
      archive: dmg                 # how to unpack that inner file
    launch: { app_bundle: "soh.app" }
    unsigned: true                 # client shows warning + requires confirm before removing quarantine
  - os: android
    arch: arm64
    asset: "SoH-*.apk"
    archive: apk
    android_package: "com.dishii.soh"
assets_required:                   # optional
  - id: rom
    label: "Ocarina of Time ROM (N64 US 1.0, or GameCube Master Quest)"
    accept:                        # at least one; sha1 or sha256 of known-good dumps
      - { sha1: "[TBD]", name: "N64 US 1.0" }
      - { sha1: "[TBD]", name: "GC MQ" }
    formats: [z64, n64, v64]       # client may byteswap n64/v64 → z64 if allowed below
    normalize: byteswap_to_z64     # optional transform before hashing/import
    import:
      mode: copy                   # copy | extract_with_port (run port's own extractor) | pick_folder
      dest: "{install}/"
      run: null                    # for extract_with_port: command template
game_files:                        # optional: files from the player's own copy of the game
  label: "Ocarina of Time"         # the game as stores sell it: "Uses your copy of …"
  stores:                          # optional: where Portside looks for an installed copy
    steam: [123456]                # Steam app ids
    gog: ["1234567890"]            # GOG product ids
    epic: ["ExampleAppName"]       # Epic app names (the launcher's AppName)
  files: ["data.pak", "music/"]    # needed, relative to the game's folder; "/" = a folder
  dest:                            # optional, per OS; "{install}/" where not given
    macos: "{install}/Example.app/Contents/Resources/"
saves:                             # optional; omit = sync unsupported
  root:
    windows: "{install}/Save/"
    linux:   "{xdg_data}/soh/Save/"
    macos:   "{app_support}/soh/Save/"
    android: "{saf:shared}/soh/Save/"   # Android: SAF-picked shared dir; see client/docs/platforms.md
  include: ["**/*.sav", "**/*.json"]
  exclude: ["**/*.log", "**/cache/**"]
  portable: true                   # bytes identical across OSes
  tied_to_version: false           # true = warn on cross-version restore (save states)
  converter: null                  # optional script id for per-OS transforms (later)
multiplayer: online_coop           # none|local|online_coop|online_pvp
online:                            # optional, for chat game invites (later)
  join_uri: null                   # e.g. "sm64coopdx://join/{lobby}"
redistribution: false              # true only with maintainer permission (legal/policy.md)
maintainers_verified: false        # set by Portside staff when port team signs off
links:
  homepage: "https://shipofharkinian.com"
  discord: null
changelog:
  - "2026-10-13: initial"
```

### `url_list`: builds on the port's own site
For ports whose builds aren't GitHub release files (ADR-0024). The manifest names one version
and its files; there is no `repo`, `channel` or `tag_pattern`.

```yaml
source:
  type: url_list
  version: "15.3"                  # required; shown as the release tag
  released: "2026-04-04"           # required, date (or RFC 3339 date-time)
  release_url: "https://www.openttd.org/news/2026/04/04/openttd-15-3"   # required, https
  hosts: [cdn.openttd.org]         # required; every download and redirect must stay on these
builds:
  - os: windows
    arch: x86_64
    asset: "openttd-15.3-windows-win64.zip"   # = the url's file name
    url: "https://cdn.openttd.org/openttd-releases/15.3/openttd-15.3-windows-win64.zip"
    sha256: "61c0a6a4..."          # required, from the site's published checksums
    size: 11250350                 # required, bytes
    archive: zip
    strip_root: true
    launch: { exe: "openttd.exe" }
```

A new version is a manifest change. `url`, `sha256` and `size` are only allowed on
`url_list` builds, where they are required.

- `archive` says how to unpack the downloaded asset. `strip_root` unwraps a single top-level
  directory.
- `dmg`: the client mounts the image read-only, copies the volume's contents (except the
  `Applications` symlink) into `{install}`, then unmounts. Launch paths are relative to the
  volume root, so a volume `DevilutionX/devilutionX.app` is `app_bundle: "devilutionX.app"`.
- `apk`, `appimage` and `none`: the asset itself is the result; nothing is unpacked. For
  `appimage` and `none` the file is saved as `launch.exe` (the asset name when there is
  none), so a versioned file name like `Port-v1.2.AppImage` keeps one launch path across
  releases. `appimage` files are made executable.
- `nested`: some ports ship an archive inside an archive (a `.dmg`, `.zip` or `.tar.gz`
  inside a `.zip`). The client unpacks the asset with `archive`, finds the one file whose
  path inside it matches `nested.file` (a glob, so `SpaceCadetPinball-*.dmg` survives a
  version bump), unpacks that with `nested.archive`, and throws the outer files away.
  `strip_root` and the launch paths apply to the inner archive. The outer `archive` must be
  `zip`, `tar.gz`, `tar.xz` or `7z`; `nested.archive` can't be `apk`.

### `game_files`: the player's own copy of the game
For ports that run on the data files of a game the player bought on a store (an engine
remake that reads the original `.dat` files, for example). It never downloads game files:
Portside only copies files that are already on the player's computer, from a copy a store
installed or from a folder the player picks. Console dumps and ROMs stay with
`assets_required`.

- `stores` lists the store ids of the game. Portside reads each store's own record of what
  is installed, on Windows, macOS and Linux (Steam Deck included):
  - Steam: `steamapps/libraryfolders.vdf` in each Steam folder, then
    `steamapps/appmanifest_<id>.acf` in each library (fully installed only), giving
    `steamapps/common/<installdir>`.
  - GOG: GOG Galaxy's database (`InstalledBaseProducts`), Heroic's
    `gog_store/installed.json`, and a `goggame-<id>.info` file in the usual GOG game
    folders.
  - Epic: the Epic Games Launcher's `Manifests/*.item` files, and Heroic's or Legendary's
    `installed.json`.
- `files` are paths inside the game's folder, with `/` between parts. One ending in `/` is a
  folder, copied with everything in it. Names match without regard to case, and are copied
  with the spelling the game's copy uses. The files may sit up to four folders down (a macOS
  app bundle, a `data` folder): Portside uses the shallowest folder that holds all of them.
- `dest` says where the files go on each OS, with the placeholders below; `{install}/` where
  it isn't given. A Windows build run through Proton uses `windows`, inside its prefix.
- When the game is installed (and again after each update, into the new folder) Portside
  copies the files from the copy it used last time, else from the first copy it finds.
  When there is none, the game page says so and offers "Choose folder". The game still
  launches without them; most ports then ask for their files themselves.

## Path placeholders
| Placeholder | Windows | macOS | Linux | Android |
|---|---|---|---|---|
| `{install}` | Portside library dir for this port+version | same | same | app-private files dir |
| `{appdata}` | `%APPDATA%` | — | — | — |
| `{localappdata}` | `%LOCALAPPDATA%` | — | — | — |
| `{documents}` | Known Folder Documents | `~/Documents` | `$XDG_DOCUMENTS_DIR` | — |
| `{app_support}` | — | `~/Library/Application Support` | — | — |
| `{xdg_data}` | — | — | `$XDG_DATA_HOME` or `~/.local/share` | — |
| `{xdg_config}` | — | — | `$XDG_CONFIG_HOME` or `~/.config` | — |
| `{home}` | `%USERPROFILE%` | `~` | `~` | — |
| `{saf:shared}` | — | — | — | user-granted SAF tree |

Flatpak builds use the port's sandboxed path: `~/.var/app/<app-id>/data/...` — declare via
`{flatpak:<app-id>}`.

## Validation rules
Implemented by `validateManifest` in `catalog/src/schema/manifest.ts` unless marked otherwise.
- Unknown fields are errors (every object is closed).
- `id` unique and equal to the file name; must exist upstream (warn, not error, for
  non-upstream entries; the catalog build skips manifests whose id is not in the catalog).
- For `github_releases`, each build's `asset` glob must match exactly one asset in the latest release. The catalog
  build warns when one doesn't, and leaves that build out of the release's assets.
- `nested` only inside `zip`, `tar.gz`, `tar.xz` or `7z`, and never `apk`.
- At least one build; no two builds with the same `os`/`arch`. (So `perfect|playable` always
  has a build.)
- Each build needs `launch.exe` or `launch.app_bundle`, except `apk` builds, which need
  `android_package` instead.
- `source.repo` is required for `github_*` sources; `tag_pattern` when `channel: tag_pattern`.
- `url_list`: `version`, `released`, `release_url` and `hosts` are required, and `repo`,
  `channel`, `tag_pattern` and `min_version` are not allowed. Each `hosts` entry must be in
  the reviewed list `URL_LIST_HOSTS` in `catalog/src/schema/manifest.ts`. Every build needs
  `url` (https, host in `hosts`, file name = `asset`), `sha256` (64 lowercase hex) and
  `size`. `version`, `released`, `release_url` and `hosts` are not allowed on other sources,
  nor `url`, `sha256` and `size` on their builds.
- `accept` entries need `sha1` or `sha256`, as lowercase hex of the correct length
  (`[TBD]` placeholders fail on purpose).
- No URLs to asset/ROM hosts: any URL ending in a ROM, disc or BIOS image extension is an
  error; host denylist [TBD].
- `game_files`: `files` are relative paths with no `..`, `\`, `*` or leading `/`; each
  `dest` starts with a placeholder and has no `..`; no store id twice.
- `redistribution: true` passes with a warning (needs maintainer permission).
- JSON Schema: `catalog/schemas/manifest.v1.json` (`pnpm schema:gen` in `catalog/`).

## Versioning
`schema` integer bumps on breaking change. Client refuses unknown major schema and shows
"update Portside".

## Changelog
- 2026-10-13 — v1 draft.
- 2026-10-13 — Validation rules made concrete (closed objects, file name = id, launch target,
  duplicate builds, hash format, ROM-extension URL check); JSON Schema generated.
- 2026-10-13 — Manifests live in the monorepo's `manifests/` folder (ADR-0011). New
  "Archives" section: how `dmg` is unpacked and where launch paths start. No schema change.
- 2026-10-14 — Validated and published by `catalog/` (ADR-0015); asset globs checked against
  the latest release at build time. No schema change.
- 2026-10-15 — `nested` build field for an archive inside the release asset; `appimage` and
  `none` files are saved as `launch.exe`. Clients before this ignore `nested` and save
  AppImages under the asset name, so those builds fail their install with "launch target
  missing" until the app is updated.
- 2026-10-16 — `url_list` source (ADR-0024): `source.version`, `released`, `release_url`,
  `hosts`; build `url`, `sha256`, `size`. Clients before this refuse `url_list` ports
  ("unsupported source").
- 2026-10-18 — `game_files`: copy a port's data files from the player's own copy of the game
  installed by Steam, GOG or Epic (or a folder they pick). Clients before this ignore it, so
  those ports keep asking for their files themselves.
