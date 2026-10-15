# Portside client

Tauri 2 desktop app (Rust core + Svelte UI). Android comes later (roadmap milestone 5).
Read `CLAUDE.md` for conventions and `docs/` for the design.

## Run it

Needs Node 22, pnpm 10, and Rust via [rustup](https://rustup.rs), not Homebrew's `rust` (the pinned toolchain in
`rust-toolchain.toml` installs itself). macOS also needs the Xcode command line tools; Linux
needs the WebKitGTK packages listed in `.github/workflows/client.yml`.

```sh
cd client
pnpm install
pnpm tauri dev              # desktop app against the published catalog (GitHub Pages)
```

`PORTSIDE_CATALOG_URL=http://localhost:3000 pnpm tauri dev` points it at another copy of
the catalog, for example a checkout of the `gh-pages` branch of
`Erickbarrera/portside-catalog` served with `npx serve`.
`pnpm --filter @portside/ui dev` runs only the UI in a browser at http://localhost:5173,
answering commands from synthetic fixtures (no Rust, no network).

## Layout

```
Cargo.toml, rust-toolchain.toml   # Rust workspace
crates/portside-core/             # domain enums, platform detection, app paths
crates/portside-api/              # typed client for the static catalog + contract test
crates/portside-catalog/          # offline SQLite copy of the catalog, ETag sync
src-tauri/                        # Tauri shell: commands, capabilities, icons
ui/                               # SvelteKit SPA (Svelte 5); ipc/bindings.ts is generated
```
