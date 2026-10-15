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
  type: github_releases            # github_releases | github_actions_artifact (later) | url_list (later)
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
    asset: "SoH-*-Linux.zip"
    archive: zip
    launch: { exe: "soh.appimage", chmod_x: true }
  - os: macos
    arch: universal
    asset: "SoH-*-Mac.zip"
    archive: zip
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

## Archives
- `archive` says how to unpack the downloaded asset. `strip_root` unwraps a single top-level
  directory.
- `dmg`: the client mounts the image read-only, copies the volume's contents (except the
  `Applications` symlink) into `{install}`, then unmounts. Launch paths are relative to the
  volume root, so a volume `DevilutionX/devilutionX.app` is `app_bundle: "devilutionX.app"`.
- `apk`, `appimage` and `none`: the asset itself is the result; nothing is unpacked.
- Some ports ship an archive inside an archive (a `.dmg` or `.zip` inside a `.zip`). Schema
  1 cannot describe that yet, so those builds are left out of manifests for now [TBD: a
  `nested` field, needs the client model changed in the same PR].

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
- Each build's `asset` glob must match exactly one asset in the latest release. The catalog
  build warns when one doesn't, and leaves that build out of the release's assets.
- At least one build; no two builds with the same `os`/`arch`. (So `perfect|playable` always
  has a build.)
- Each build needs `launch.exe` or `launch.app_bundle`, except `apk` builds, which need
  `android_package` instead.
- `source.repo` is required for `github_*` sources; `tag_pattern` when `channel: tag_pattern`.
- `accept` entries need `sha1` or `sha256`, as lowercase hex of the correct length
  (`[TBD]` placeholders fail on purpose).
- No URLs to asset/ROM hosts: any URL ending in a ROM, disc or BIOS image extension is an
  error; host denylist [TBD].
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
