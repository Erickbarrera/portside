# Build & release

## Toolchain
Rust stable (pinned), Node 22, pnpm 9+, Tauri CLI 2.x. Android: SDK, NDK, Java 21, and
`rustup target add aarch64-linux-android x86_64-linux-android`.

## CI (GitHub Actions, `.github/workflows/client.yml`, path filter `client/**`)
- PR (built): fmt, clippy, cargo test (incl. the catalog contract test and a check that
  `ui/src/lib/ipc/bindings.ts` is current), Biome, svelte-check, vitest, UI build, on
  ubuntu-24.04 with the WebKitGTK packages; the `android` job runs clippy for
  `aarch64-linux-android` with the runner's NDK. [TBD: debug app build in CI]
- Release: `.github/workflows/client-release.yml`, see "Publishing a release" below.
  It builds Android too (below).

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
   - Android: `Portside_<version>_android.apk`, arm64 + x86_64 (ubuntu-24.04, see "Android")
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
- **Tauri updater key** (free): enables in-app updates, see "Auto-update" below.

The app reads the static catalog from `https://erickbarrera.github.io/portside`
(docs/specs/static-catalog.md). A build can bake in another address from the repository
variable `PORTSIDE_DEFAULT_CATALOG_URL` (Settings → Secrets and variables → Actions →
Variables); `PORTSIDE_CATALOG_URL` still overrides it at run time.

## Android
`pnpm tauri android build --apk --target aarch64 --target x86_64` builds an unsigned
release APK from `src-tauri/gen/android` (committed; regenerate with `pnpm tauri android
init` only when Tauri asks, then put back `AppsPlugin.kt` and the manifest changes). The
workflow aligns it and signs it with `apksigner`. On a phone with USB debugging:
`pnpm tauri android dev` (needs `ANDROID_HOME`, `NDK_HOME`).

Android installs an update only when it is signed with the same key as the installed copy,
so every release has to be signed with one key that is never lost:
1. Once, on your computer: `keytool -genkeypair -keystore portside-release.jks -alias
   portside -keyalg RSA -keysize 4096 -validity 10000 -dname "CN=Portside"`, with a password.
   Keep the file and password somewhere safe: without them no installed copy can update
   again until players uninstall and reinstall.
2. Repository → Settings → Secrets and variables → Actions → secrets:
   `ANDROID_KEYSTORE` (`base64 -i portside-release.jks`), `ANDROID_KEYSTORE_PASSWORD`, and
   `ANDROID_KEY_ALIAS` if it isn't `portside`.

Without `ANDROID_KEYSTORE` the workflow signs with a throwaway key made for that run and
warns: the APK installs, but the next release can't update it in place.
No in-app updates on Android yet: players download new APKs (or use Obtainium).

## Versioning
SemVer, `client-vX.Y.Z` tags. One channel for now: every published release counts as an
update, pre-releases included (all 0.x releases are pre-releases). [TBD: a stable/beta choice
once 1.0 ships]

## Auto-update
`tauri-plugin-updater` with Ed25519-signed updates (`src-tauri/src/app_update.rs`).
- **Finding an update.** On start (after a few seconds), every 6 hours, and from Settings →
  Updates → Check for updates, the app lists this repository's releases through the GitHub API
  (anonymous, one request) and takes `latest.json` from the newest published `client-v*`
  release that has one. GitHub's `releases/latest/download/` link can't be used: it skips
  pre-releases. The plugin then compares versions, so an older or equal release is no update.
- **Installing.** A banner offers "Update and restart" (or Later; Settings keeps offering
  it). It refuses while a game runs or installs. The plugin downloads the file for this
  install type, checks its signature against the public key built into the app
  (`requireSignedVersion` on, so an old signed build can't be passed off as a new version),
  installs and restarts: macOS replaces the `.app`, Windows runs the NSIS/MSI installer
  (passive), Linux rewrites the AppImage in place (Steam Deck: Add to Steam shortcuts keep
  working) or installs the `.deb`/`.rpm` through pkexec (asks for the password).
- **Which builds update.** Only release builds with the public key. `pnpm tauri dev`, debug
  builds and builds made without the key say so in Settings and never check.
- **Signing releases.** The release workflow's "Updater signing" step, when the key is set up,
  passes `--config updater.conf.json` (`bundle.createUpdaterArtifacts` and
  `plugins.updater.pubkey`) to the build; tauri-action then uploads the `.sig` files and
  `latest.json` to the release. Releases from before the key (up to 0.2.0) have no
  `latest.json`: players on them update by hand once.

### Setting up the updater key (once)
1. On your computer: `pnpm tauri signer generate -w ~/.tauri/portside-updater.key` (in
   `client/`), with a password. Keep the private key file and password somewhere safe: losing
   them means no installed copy can update again until players reinstall by hand.
2. Repository → Settings → Secrets and variables → Actions:
   - secret `TAURI_SIGNING_PRIVATE_KEY`: the contents of `~/.tauri/portside-updater.key`;
   - secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: its password;
   - variable `PORTSIDE_UPDATER_PUBKEY`: the contents of `~/.tauri/portside-updater.key.pub`.
3. The next release is signed and carries `latest.json`; copies from that release on update
   themselves.

Flatpak/F-Droid builds would disable the in-app updater (store updates instead).

## Secrets needed [TBD]
Apple Developer ID cert + notarization creds, Windows signing, Tauri updater key (steps above),
Android keystore (steps above).

