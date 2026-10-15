# Build & release

## Toolchain
Rust stable (pinned), Node 22, pnpm 9+, Tauri CLI 2.x. Android: SDK, NDK, Java 17.

## CI (GitHub Actions, `.github/workflows/client.yml`, path filter `client/**`)
- PR (built): fmt, clippy, cargo test (incl. the catalog contract test and a check that
  `ui/src/lib/ipc/bindings.ts` is current), Biome, svelte-check, vitest, UI build, on
  ubuntu-24.04 with the WebKitGTK packages. [TBD: debug app build in CI]
- Release: `.github/workflows/client-release.yml`, see "Publishing a release" below.
  Android (apk) and the updater manifest come later.

## Publishing a release
1. Bump the version in `src-tauri/tauri.conf.json` **and** `[workspace.package]` in
   `Cargo.toml` (they must match), merge to `main`.
2. Fill in "What's new" in `.github/client-release-notes.md`. Tag that commit and push the tag
   (or run the workflow from the Actions tab with the `tag` input, which releases the current
   commit of `main` and creates the tag when the release is published): `git tag client-v0.1.0 && git push origin client-v0.1.0`.
3. The workflow checks the tag matches the version, creates a **draft** GitHub release
   (notes from `.github/client-release-notes.md`) and builds in parallel:
   - macOS universal (arm64 + x86_64): `.dmg` (macos-latest)
   - Windows x64: NSIS `-setup.exe` and `.msi` (windows-latest)
   - Linux x64: `.AppImage`, `.deb`, `.rpm` (ubuntu-22.04 for older glibc / Steam Deck)
4. When every build succeeded, the `publish` job publishes the release: 0.x versions and
   versions with a pre-release part (`-beta.1`) as **pre-releases**, the rest as the latest
   release. Edit "What's new" on GitHub afterwards if needed. If a build fails, the release
   stays a draft; re-running the failed job reuses it and publishes when it passes.
   The website's download button reads the newest published release, pre-releases included.

A pull request that changes the workflow file, or a manual run (Actions tab → client-release
→ Run workflow), builds the same files as 7-day workflow artifacts without touching Releases.

### Signing (not set up yet)
Builds are unsigned. macOS builds are ad-hoc signed (`bundle.macOS.signingIdentity = "-"` in
`tauri.conf.json`) so Apple Silicon Macs show "Open Anyway" instead of "damaged"; users still
pass Gatekeeper by hand, Windows users click through SmartScreen. What each secret adds:
- **Apple Developer ID cert + notarization** (Apple Developer Program, paid): no Gatekeeper
  warning. Secrets `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
  `APPLE_ID`, `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID`, passed as env to tauri-action.
- **Windows code signing** ([TBD: Azure Trusted Signing or OV cert]): fewer SmartScreen warnings.
- **Tauri updater key** (free, `pnpm tauri signer generate`): enables in-app auto-update;
  secrets `TAURI_SIGNING_PRIVATE_KEY` (+ password), and tauri-action then uploads `latest.json`.

The app reads the static catalog from `https://erickbarrera.github.io/portside-catalog`
(docs/specs/static-catalog.md). A build can bake in another address from the repository
variable `PORTSIDE_DEFAULT_CATALOG_URL` (Settings → Secrets and variables → Actions →
Variables); `PORTSIDE_CATALOG_URL` still overrides it at run time.

## Versioning
SemVer, `client-vX.Y.Z` tags. Channels: stable, beta (updater endpoint per channel).

## Auto-update
`tauri-plugin-updater` with Ed25519-signed manifests. Private key in CI secrets only.
Flatpak/F-Droid builds disable the in-app updater (store updates instead).

## Secrets needed [TBD]
Apple Developer ID cert + notarization creds, Windows signing, Tauri updater key, Android
keystore.

