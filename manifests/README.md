# Port manifests

One file per port: `ports/<id>.yaml`, where `<id>` is the port's id in
[awesome-decomp-games](https://github.com/SubmitGame/awesome-decomp-games). A manifest tells
Portside which upstream release file to download for each OS and how to launch it.
Format: [`docs/specs/manifest-format.md`](../docs/specs/manifest-format.md). They live next to
the catalog builder that reads them (ADR-0011).

## Ports
| Port | macOS | Windows | Linux |
|---|---|---|---|
| `harbourmasters-shipwright` | universal (unsigned, nested dmg) | x86_64 | x86_64 (AppImage) |
| `2ship2harkinian-2ship2harkinian` | universal (unsigned, nested dmg) | x86_64 | x86_64 (AppImage) |
| `zelda64recomp-zelda64recomp` | universal (unsigned, nested zip) | x86_64 | x86_64, arm64 (nested tar.gz) |
| `banjorecomp-banjorecomp` | universal (unsigned) | x86_64 | x86_64, arm64 |
| `hedge-dev-unleashedrecomp` | — (no upstream build) | x86_64 | — (Flatpak bundle only) |
| `coop-deluxe-sm64coopdx` | arm64, x86_64 (unsigned) | x86_64 | x86_64 |
| `harbourmasters-starship` | — (no upstream build) | x86_64 | x86_64 (AppImage) |
| `harbourmasters-spaghettikart` | arm64, x86_64 (unsigned) | x86_64 | x86_64 (AppImage) |
| `twilitrealm-dusklight` | arm64, x86_64 (signed) | x86_64 | x86_64, arm64 (AppImage) |
| `diasurgical-devilutionx` | universal (signed) | x86_64 | x86_64 (AppImage), arm64 |
| `alexbatalov-fallout1-ce` | universal (signed) | x86_64 | x86_64 |
| `openrct2-openrct2` | universal (unsigned) | x86_64 | x86_64 (AppImage) |
| `openra-openra` | universal (signed) | x86_64 | x86_64 (Red Alert AppImage) |
| `openmw-openmw` | arm64, x86_64 (unsigned) | — (installer only) | x86_64 |
| `vcmi-vcmi` | arm64, x86_64 (signed) | x86_64 | x86_64, arm64 (AppImage) |
| `k4zmu2a-spacecadetpinball` | universal (unsigned, nested dmg) | x86_64 | — (no upstream build) |
| `isledecomp-isle-portable` | arm64, x86_64 (unsigned) | x86_64 | x86_64 (AppImage) |
| `dethrace-labs-dethrace` | universal (unsigned) | x86_64 | x86_64, arm64 |
| `corsixth-corsixth` | universal (signed) | x86_64 | x86_64 (AppImage) |
| `bvschaik-julius` | universal (unsigned) | x86_64 | x86_64 (AppImage) |

Not covered yet, with the reason (checked 2026-10-15):
- `snesrev-zelda3`: Windows only, and the player must first apply a patch file (`.bps`) to
  their ROM to make the game's asset file; Portside can't run that step yet.
- `openttd-openttd`, `sfttech-openage`, `terrycavanagh-vvvvvv`: only source code in their
  GitHub releases (OpenTTD hosts builds on its own site, which the installer can't download
  from).
- `rsdkmodding-sonic-mania-decompilation`: releases ship only the game logic library, not a
  runnable game (it needs the RSDKv5 engine built separately).
- `sonicnext-dev-marathonrecomp`: no release yet.

Every port needs files from the player's own copy of the game; `status_notes` says which.

## Adding a port
1. Only official builds from the upstream project's own GitHub releases. Never link to game
   files (docs/legal/policy.md).
2. Open the release's real files and check what the manifest claims: the asset name each
   `asset` glob matches (exactly one per build), the archive layout (`strip_root`), the
   launch target (for an AppImage, the name it is saved as), and for macOS whether the app
   is signed with a Developer ID (if not, set `unsigned: true`).
3. Note the release you checked against in a comment at the top of the file.
4. An archive inside the asset (a `.dmg`, `.zip` or `.tar.gz` inside a `.zip`) goes in
   `nested`. Leave out builds Portside can't install (installers, Flatpak bundles, `.7z`).
5. Run the checks in `catalog/`: `pnpm validate` (every file here), then
   `pnpm build --out dist` to build a catalog with it and see warnings about asset globs that
   match no file in the latest release (needs network; set `GITHUB_TOKEN` if you hit the
   GitHub rate limit). The published catalog picks the change up on the next hourly build
   after it merges.

## Covers
`covers.yaml` fixes SteamGridDB covers for games that have no other image: map a `game_key`
to the right SteamGridDB game id, or to `none` for no cover. The next hourly build applies it.
