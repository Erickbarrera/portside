# Port manifests

One file per port: `ports/<id>.yaml`, where `<id>` is the port's id in
[awesome-decomp-games](https://github.com/SubmitGame/awesome-decomp-games). A manifest tells
Portside which upstream release file to download for each OS and how to launch it.
Format: [`docs/specs/manifest-format.md`](../docs/specs/manifest-format.md). They live next to
the catalog builder that reads them (ADR-0011).

## Ports
[`COVERAGE.md`](COVERAGE.md) lists every port with a manifest and its builds per OS, and why
the other catalog ports have none (most publish no release files at all).

The first 20 manifests (2026-10-13 to 2026-10-15) were written and checked by hand. The rest
were generated on 2026-10-16 from each port's latest GitHub release and kept only where the
build installed in a check run of the app's installer (download, checksum, unpack, launch
file present and built for the right OS); they have not been played. Each generated file
says so in its first line. A generated manifest that launches the wrong program or misses a
build is fixed by hand like any other.

Ports that publish builds on their own site use a `url_list` source (ADR-0024):
`openttd-openttd` so far. Candidates looked at on 2026-10-16, not added yet because their
builds haven't been through an install check: Doomsday Engine (`files.dengine.net`:
Windows x64 zip, signed x86_64 Mac dmg), Freeciv (`files.freeciv.org`: Linux x86_64
AppImage) and Yamagi Quake II (`deponie.yamagi.org`: Windows zip). Each needs its host
added to `URL_LIST_HOSTS`.

Not covered, with the reason (checked 2026-10-15):
- `snesrev-zelda3`: Windows only, and the player must first apply a patch file (`.bps`) to
  their ROM to make the game's asset file; Portside can't run that step yet.
- `sfttech-openage`, `terrycavanagh-vvvvvv`: only source code in their GitHub releases.
- `rsdkmodding-sonic-mania-decompilation`: releases ship only the game logic library, not a
  runnable game (it needs the RSDKv5 engine built separately).

Every port needs files from the player's own copy of the game unless it is a full remake
(OpenTTD); hand-written manifests say which in `status_notes`.

## Adding a port
1. Only official builds from the upstream project's own GitHub releases, or from its own
   download site through a `url_list` source on a reviewed host (ADR-0024, `URL_LIST_HOSTS`
   in `catalog/src/schema/manifest.ts`). Never link to game files (docs/legal/policy.md).
2. Open the release's real files and check what the manifest claims: the asset name each
   `asset` glob matches (exactly one per build), the archive layout (`strip_root`), the
   launch target (for an AppImage, the name it is saved as), and for macOS whether the app
   is signed with a Developer ID (if not, set `unsigned: true`).
3. Note the release you checked against in a comment at the top of the file.
4. An archive inside the asset (a `.dmg`, `.zip` or `.tar.gz` inside a `.zip`) goes in
   `nested`. Leave out builds Portside can't install (installers, Flatpak bundles, `.deb` or
   `.rpm` packages), 32-bit Linux builds, and DOS programs.
5. Android: only an APK the project publishes itself (`archive: apk`), with
   `android_package` set to the app's `applicationId` from its Gradle file. One APK with
   every ABI is `universal`; per-ABI files are `arm64` and `x86_64` builds.
6. Run the checks in `catalog/`: `pnpm validate` (every file here), then
   `pnpm build --out dist` to build a catalog with it and see warnings about asset globs that
   match no file in the latest release (needs network; set `GITHUB_TOKEN` if you hit the
   GitHub rate limit). The published catalog picks the change up when it merges.

## Covers
`covers.yaml` fixes SteamGridDB covers for games that have no other image: map a `game_key`
to the right SteamGridDB game id, or to `none` for no cover. Merging it republishes the catalog.
