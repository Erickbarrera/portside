# Spec: Catalog & playable status

## Upstream source
`https://raw.githubusercontent.com/SubmitGame/awesome-decomp-games/main/data/games.json`
(CC0 data). The file is an object whose `games` array holds the entries (older copies were a
bare array; the ingest accepts both). Snapshot 2026-10-13: 2,046 entries, 2,033 unique ids,
1,817 unique games. Types: decompilation 836, static-recomp 469, disassembly 234,
source-port 221, engine-remake 221, browser-port 57, tool 8.

Ingest rules:
- An entry needs a non-empty `id`, `title`, `game_key` and a known `type`; others are dropped.
- Duplicate ids: keep the entry with the most stars. (2026-10-13: one id appears 14 times.)
- `platform` is split on `/` into the game's original platforms ("N64 / GameCube").
- Game title = title of the game's most-starred port, minus a trailing "(…)".
- `screenshot` becomes `screenshot_url`, linked from the list's repo; ports without one use
  their repo's own preview image, then the game's SteamGridDB cover (static-catalog.md,
  "Screenshots"; ADR-0019, ADR-0020).

Relevant upstream fields: `id`, `title`, `game_key`, `platform` (ORIGINAL platform, e.g.
"N64" — not target OS), `type`, `url`, `repo`, `description`, `stars`, `forks`,
`last_commit`, `archived`, `license`, `release`, `release_date`, `release_url`,
`releases`, `homepage`, `play_url` (browser ports), `needs_assets` (mostly null),
`multiplayer`, `ai_flag`, `ratings{playability,completeness,activity,popularity}` (1–5),
`score`, `screenshot` (repo-relative path), `video`, `note`, `top_pick`.

Upstream does not say which OSes a port runs on → that comes from Portside manifests.

## Merge model
`catalog_entry = upstream ⊕ manifest ⊕ curator overrides ⊕ aggregated compat reports`.
Precedence for each field: curator override > manifest > upstream.

## Playable status
| Status | Meaning | Seed from upstream when no manifest |
|---|---|---|
| `perfect` | Complete, no known issues | playability=5 and completeness=5 |
| `playable` | Beatable start to finish, minor bugs | playability=5 |
| `in_progress` | Plays, missing content or crashes | playability 3–4 |
| `boots` | Menus/levels load, not really playable | playability 2 |
| `not_playable` | No runnable build | type in (decompilation, disassembly) and no release; also playability 1 or unrated |

Rules are applied top to bottom: the decompilation/disassembly rule first, then by
playability.

Status is per port and per OS (`os_status`): a build can be `playable` on Windows and
`boots` on macOS. Compat reports (lexicon `gg.portside.feed.report`) adjust the displayed
confidence, not the status; curators change status.

## Actions shown by type
- Has manifest build for current OS → **Install**
- `browser-port` with `play_url` → **Play in browser** (embedded webview, sandboxed)
- Otherwise → **View project** + **Follow** (notify when a build appears)

## Refresh cadence
The catalog is rebuilt and published as static files (static-catalog.md, ADR-0015):
- games.json, manifests and GitHub releases: once a day, and on every push that changes
  `catalog/` or `manifests/`. Releases are read with the workflow's own token.
- Link health: [TBD: not built].

## Changelog
- 2026-10-17 — Rebuilt once a day instead of hourly.
- 2026-10-14 — Built into a static catalog hourly instead of ingested by a worker
  (ADR-0015). Curator overrides and compat reports are not implemented.
- 2026-10-13 — Upstream file is now `{ games: [...] }`; ingest rules (validity, duplicate ids,
  platforms, game title, screenshots unpublished); seed rule for playability 1 / unrated.
