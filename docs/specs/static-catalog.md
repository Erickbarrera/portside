# Spec: Static catalog (v1)

Status: Draft · Producer: `catalog/` (published by `.github/workflows/catalog.yml`) ·
Consumers: desktop app (`client/crates/portside-api`), website (`web/src/lib/catalog.ts`).
Decision: ADR-0015.

The catalog is a tree of JSON files on a static host. There is no server: no search
endpoint, no query parameters, no auth. Field-level shapes are the TypeBox schemas in
`catalog/src/schema/catalog.ts`; `pnpm contract:write` (in `catalog/`) writes them as an
OpenAPI 3.1 document, [`static-catalog/v1.json`](static-catalog/v1.json), which the desktop
app's contract test reads. CI fails when that file is stale.

## Where
Base URL: `https://erickbarrera.github.io/portside-catalog` (GitHub Pages of the public
`portside-catalog` repository; the private repository's workflow pushes the files to its
`gh-pages` branch)
[TBD: custom domain]. Clients take the base URL from configuration
(`PORTSIDE_CATALOG_URL` in the app, `PUBLIC_PORTSIDE_CATALOG_URL` on the website).

GitHub Pages serves every file with `Access-Control-Allow-Origin: *`, an `ETag`, gzip, and
`Cache-Control: max-age=600`. Any static host with CORS and ETags works the same.

## Files
All under `v1/`. Field names are snake_case. A missing file is a plain 404.

| Path | Body |
|---|---|
| `v1/index.json` | `{ schema: 1, rev, generated_at, port_count, content_hash }` |
| `v1/snapshot.json` | `{ rev, ports: PortSummary[] }`, ports sorted by `id` |
| `v1/ports/{id}.json` | `PortDetail`: summary + upstream fields, manifest, up to 5 latest non-revoked releases |
| `v1/ports/{id}/releases.json` | `Release[]`, newest first (up to 10), revoked ones included with `revoked: true` |
| `v1/games/{game_key}.json` | `Game`: title, original platforms, ports by stars |

`{id}` and `{game_key}` are used as-is (they match `^[a-z0-9][a-z0-9._-]*$`, case-insensitive);
a game whose key doesn't match gets no `games/` file.

## Revisions and change detection
- `rev` is the Unix time (seconds) of the build that last changed any port summary. A build
  whose summaries equal the published ones keeps the published `rev`, so `snapshot.json`
  stays byte-identical and its ETag stays the same.
- `updated_at` on a port is when that port's summary last changed (detail-only changes,
  like a new release, don't move it).
- `content_hash` (sha256 over every other file) decides whether a build is published at
  all; an unchanged catalog is not redeployed.
- The builder reads the published `index.json` and `snapshot.json` to carry `rev` and
  `updated_at` forward. If they can't be read (first build), everything counts as changed.

### Client sync
Clients keep a copy of `snapshot.json` and its `ETag`. Refresh = `GET v1/snapshot.json`
with `If-None-Match`; 304 means current. A 200 replaces the copy. A 200 with the same
`rev` as the copy (a re-deploy under a new ETag) only updates the stored ETag. Search runs
on the copy.

## Releases
For each manifest with `source.type: github_releases`, the build reads
`GET /repos/{repo}/releases?per_page=30` and keeps the newest 10 that the manifest's channel
selects: `stable` (no drafts, no prereleases), `prerelease` (no drafts) or `tag_pattern`
(no drafts, tag matches the regex). Each build in the manifest becomes a `ReleaseAsset`
when its `asset` glob matches exactly one file name (`*`, `?`, `[...]`); `sha256` comes
from GitHub's asset `digest` when present, else null. Release `id` is GitHub's release id.
`revoked` is always false for now [TBD: revocation list]. When GitHub can't be reached for
a port, the build keeps that port's published `releases.json`.

For `source.type: url_list` (ADR-0024) the build reads nothing: the manifest's one version
becomes the port's one release. `id` is `url_list:{version}`, `tag` and `name` are
`version`, `published_at` is `released` (midnight UTC for a plain date), `prerelease` is
false, `html_url` is `release_url`, and each build becomes an asset with its `url`, `size`
and `sha256` and `name` = `asset`.

## Screenshots
`screenshot_url` is a link, never a copy (docs/legal/policy.md rule 6, ADR-0019):
1. The upstream list's `screenshot`: a path in its repo becomes
   `https://raw.githubusercontent.com/SubmitGame/awesome-decomp-games/main/<path>`; an
   https URL stays as is. Only `.jpg`, `.jpeg`, `.png`, `.webp` and `.gif` pass.
2. Else the custom social preview image the port's own repo set on GitHub
   (`usesCustomOpenGraphImage`, read with the GraphQL API and the workflow's token), on
   `repository-images.githubusercontent.com`. GitHub's generated repo card is not used.
3. Else the game's cover from [SteamGridDB](https://www.steamgriddb.com) (ADR-0020): the
   game whose name equals the game title (ignoring case and punctuation), its best-scored
   static landscape grid (920×430 or 460×215, no NSFW or humor images), linked from
   SteamGridDB's CDN (`*.steamgriddb.com`). One cover per `game_key`, shared by its ports
   that have no image of their own. `manifests/covers.yaml` fixes wrong matches
   (`game_key: <SteamGridDB game id>`) or turns a cover off (`game_key: none`).
   A `covers.yaml` entry wins over sources 1 and 2 as well: every port of that game gets the
   chosen SteamGridDB cover, or null for `none` (and until the cover has been looked up).
   That is how a wrong list screenshot or repo preview image is replaced.
4. Else null. When the preview images can't be read (no token, a GitHub error), each port
   keeps its published `screenshot_url`.

SteamGridDB lookups need the `STEAMGRIDDB_API_KEY` secret. Each build looks up at most 150
games (new ones first; covers found are re-checked after 30 days, misses after 7) and
publishes what it found as `build/steamgriddb.json`, which the next build reads back. That
file is the builder's cache, not part of the contract: apps and the website don't read it.

## Merge rules
As in catalog.md ("Merge model"), without curator overrides: a manifest's `display_name`
and `status` replace the upstream title and seed status, and each OS the manifest ships a
build for gets an `os_status` row (`source: "manifest"`). Ports without a manifest have no
`os_status` rows. A manifest whose `id` isn't in games.json is reported and skipped. Any
invalid manifest fails the build, so a bad manifest never reaches the published catalog.

## Changelog
- 2026-10-16 — Releases for `url_list` manifests come from the manifest (section
  "Releases"). No shape change.
- 2026-10-15 — A `manifests/covers.yaml` entry replaces the list screenshot and repo preview
  image too (section "Screenshots"). No shape change.
- 2026-10-14 — SteamGridDB covers as the third `screenshot_url` source, and the builder's
  cache `build/steamgriddb.json` (section "Screenshots"). No shape change.
- 2026-10-14 — `screenshot_url` is filled (section "Screenshots"). No shape change.
- 2026-10-14 — v1. Replaces the hosted catalog API (`/v1/catalog/snapshot`, `/v1/ports/...`,
  `/v1/games/...`, `/v1/search`) from api-surface.md. Snapshot deltas (`?since=`) and
  server search are gone; the snapshot has no `removed_port_ids`.
