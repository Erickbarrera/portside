# Portside client — instructions for Claude Code

Cross-platform launcher: Windows, macOS, Linux (incl. Steam Deck), Android.
Read the root `CLAUDE.md` first, then this file, then `docs/README.md`.

## Stack (ADR-0002)
- Tauri 2 (desktop + Android). Rust stable (pin in `rust-toolchain.toml`).
- UI: Svelte 5 (runes) + TypeScript + Vite in `ui/`. SvelteKit in SPA mode (`adapter-static`,
  `ssr = false`) for routing.
- Styling: CSS variables (design tokens in `docs/ui.md`), no CSS framework.
- State: Svelte stores for UI; source of truth for catalog/library/sync lives in Rust core,
  exposed via Tauri commands + events.
- The UI only uses the generated IPC bindings; catalog types come from Rust.
- pnpm workspace rooted in `client/` (`client/pnpm-workspace.yaml`, package `ui` is
  `@portside/ui`).

## Layout
Built so far: `Cargo.toml`, `rust-toolchain.toml`, `crates/portside-core`,
`crates/portside-api` (catalog endpoints only), `crates/portside-catalog`,
`crates/portside-installer`, `crates/portside-launcher`, `crates/portside-atproto`,
`crates/portside-social`, `crates/portside-sync`, `crates/portside-steam`, `src-tauri`, `ui`.
The rest is planned. `rust-version` in `Cargo.toml` must match `rust-toolchain.toml`.
```
client/
  Cargo.toml                 # workspace
  rust-toolchain.toml
  crates/
    portside-core/           # domain types, config, paths, errors
    portside-catalog/        # local SQLite cache, ETag sync of the static catalog
    portside-installer/      # release resolution, download (resume), verify, unpack
    portside-assets/         # asset import wizard backend, hashing, normalize (byteswap)
    portside-launcher/       # launch command, spawn, wait, stop (play time lives in library.db)
    portside-sync/           # cloud saves: plain-file vault in a synced folder (ADR-0023)
    portside-atproto/        # OAuth client (DPoP/PKCE/PAR), session, authenticated XRPC
    portside-social/         # friends, Bluesky DMs, game invites, boards (ADR-0021)
    portside-steam/          # Add to Steam (shortcuts.vdf, grid artwork), finding Proton (docs/platforms.md)
    portside-api/            # typed client for the static catalog (docs/specs/static-catalog.md)
  src-tauri/                 # Tauri app: commands, events, plugins, capabilities, updater
  ui/                        # Svelte app
  android/                   # Kotlin Tauri plugins (APK install, SAF) — generated + custom
  docs/
```
Crates depend downward only: `src-tauri → feature crates → portside-core`. Feature crates
never depend on Tauri; they must be testable without a window.

## Commands (run in `client/`; keep in sync)
- `pnpm install` once, then `pnpm tauri dev` — desktop app against `PORTSIDE_CATALOG_URL`
  (default: the published catalog, `https://erickbarrera.github.io/portside-catalog`).
  `PORTSIDE_DATA_DIR` overrides where it keeps its files (handy for a clean test run).
- Rust must come from rustup (it reads `rust-toolchain.toml`); Homebrew's `rust` is too old.
- `pnpm --filter @portside/ui dev` — UI only in a browser, commands answered by synthetic
  fixtures (`ui/src/lib/mock/`); installs are simulated. Add `?mock-os=macos` to the first
  URL to preview another system (e.g. the unsigned-app warning); `?mock-crash` makes games
  quit with an error after a moment; the mock is signed in to a synthetic Bluesky account
  (`?mock-signed-out` starts signed out); cloud saves start off (`?mock-saves=on` starts set
  up, `?mock-saves=conflict` gives games a save conflict on Play); `?mock-installed` starts
  with every game that has a build for that system installed; Steam is found and closed
  (`?mock-steam=running`, `launched` as if Steam started Portside, `none`), and
  `?mock-play=<port>` starts as if a Steam shortcut asked for that game; `?mock-couch` starts as
  if on Steam's Game Mode screen (couch mode on Auto turns on); `?mock-sd` is a Steam Deck with
  an SD card in (installs ask where to go); `?mock-deck` is a Steam Deck (A on a text field asks for Steam's
  keyboard); `?mock-proton` installs Windows builds as if through Proton (`=none`: Proton
  missing). `PORTSIDE_PROTON=<folder>` picks which Proton the real app uses. `PORTSIDE_SD_CARD=<folder>` makes the real app
  treat a folder as the Deck's SD card
- `pnpm tauri build --debug --no-bundle` — local build with the UI bundled
- Releases: push a `client-vX.Y.Z` tag (see docs/build-release.md, "Publishing a release")
- `cargo test --workspace` · `cargo clippy --workspace --all-targets -- -D warnings` ·
  `cargo fmt --all --check`
- `cargo test -p portside-desktop` also regenerates `ui/src/lib/ipc/bindings.ts` (commit it)
- `pnpm --filter @portside/ui test` (Vitest) · `pnpm --filter @portside/ui check`
  (svelte-check) · `pnpm lint` (Biome, TS/JSON only)
- `pnpm tauri android dev` — Android (later; needs SDK/NDK, see docs/build-release.md)

## Rules
- Rust: `thiserror` for library errors, `anyhow` only in src-tauri. No `unwrap()` outside
  tests. Async = tokio. HTTP = reqwest (rustls). SQLite = rusqlite (bundled) with
  `refinery` migrations in `crates/<crate>/migrations/` (ADR-0012).
- Every Tauri command is declared in `src-tauri/src/commands/*.rs`, typed with `specta` +
  `tauri-specta` (pinned rc, ADR-0010) so TS bindings are generated into
  `ui/src/lib/ipc/bindings.ts`. Never hand-write IPC types. Register new commands in
  `specta_builder()` in `src-tauri/src/lib.rs`. Errors return `CommandError { code, message }`.
- The catalog contract test (`crates/portside-api/tests/contract.rs`) reads
  `../docs/specs/static-catalog/v1.json`, a copy of the document the catalog builder writes
  (`pnpm contract:write` in the private planning repository).
  When the catalog changes a file shape, update the model so the test passes; models must
  round-trip the JSON exactly. There is no Portside server (ADR-0015).
- Tauri capabilities: least privilege per window (`src-tauri/capabilities/*.json`). The UI
  must not get raw fs/shell access; all file and process work goes through Rust commands.
- Paths: always via `portside-core::paths` (resolves manifest placeholders per OS). Never
  hardcode OS paths elsewhere.
- Downloads only from GitHub release hosts (github.com, objects.githubusercontent.com,
  release-assets.githubusercontent.com; `HostPolicy::github()` in `portside-installer`,
  ADR-0014). Every redirect is checked too. Exception: a `url_list` manifest (ADR-0024)
  downloads only from its own `source.hosts` (`HostPolicy::with_hosts`), which the catalog
  builder accepts only from a reviewed list, and always with a sha256.
- Never upload or log contents/paths of imported game assets.
- Secrets (OAuth tokens) only via `keyring` crate / Android Keystore.
- Social features talk to Bluesky through `portside-social`; no Portside server
  (ADR-0016, ADR-0021). Never log tokens or message text.
- UI copy: plain, short; statuses use the exact names in docs/specs/catalog.md.
- UI fixtures are synthetic ("Example …" ports): no real game names, art or links.
- Tests: unit tests per crate; integration tests use a local mock GitHub server
  (`wiremock`) and synthetic "game" archives. No real ports downloaded in CI.

## Where to read
- Module responsibilities & IPC: `docs/architecture.md`
- Install/launch/update flows: `docs/install-launch.md`
- Save sync client side: `docs/save-sync.md` (+ `../docs/specs/save-sync-protocol.md`)
- Accounts & social: `docs/accounts-social.md`
- Platform specifics (Deck, macOS quarantine, Android SAF/APK): `docs/platforms.md`
- Screens & design tokens: `docs/ui.md`
- Build, signing, release, auto-update: `docs/build-release.md`
- Testing strategy: `docs/testing.md`
