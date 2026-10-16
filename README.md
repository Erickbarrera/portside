# Portside

A launcher for community game ports: decompilations, static recompilations, source ports and
engine remakes. Browse the catalog, see which ports are playable on your system, and install
and update them from their own GitHub releases in one click. Bring your own game files:
Portside never downloads or shares them.

**Status: alpha.** Downloads are on the [releases page](https://github.com/Erickbarrera/portside/releases)
and the [website](https://erickbarrera.github.io/portside-catalog/). Builds are not signed yet;
the release notes say how to open them.

This repository holds the desktop app (Tauri 2: Rust core and Svelte UI) in `client/`. The
catalog it reads is published at `https://erickbarrera.github.io/portside-catalog`
(format: `docs/specs/static-catalog.md`).

## Features

What the app does today, and what is being built. Ticked items are in `main`; unticked items
are in progress or planned. Planned work may change.

### Catalog and library
- [x] Browse and search the catalog, with playable status and per-OS support
- [x] Works offline from a cached copy of the catalog
- [x] Game pages with cover art and screenshots
- [x] Library of installed games, also listed in the sidebar
- [ ] Install manifests for many more ports in the catalog (in progress)

### Install and play
- [x] One-click install from a port's GitHub release, with SHA-256 checks and resumable downloads
- [x] Archives: zip, tar.gz, tar.xz, dmg, AppImage, and archives inside archives
- [x] Launch and stop games, with play time and a log of each run
- [x] Uninstall (play time is kept)
- [ ] Installs from a port's own website, not only GitHub (in progress)
- [ ] `.7z` archives (in progress)
- [ ] Fix for installs from Mac disk images (in progress)
- [ ] Updates, rollback and a downloads view
- [ ] Import wizard for ports that need your own game files
- [ ] Mod management: a Mods tab per game (planned)

### Cloud saves
- [x] Sync saves between devices through a folder your cloud app already syncs
      (iCloud Drive, Dropbox, OneDrive, Google Drive, Syncthing). No Portside server.
- [x] Sync before launch and after exit, with a choice when two devices disagree
- [ ] Save locations for more ports (most still need you to pick the folder once)

### Social, on Bluesky
- [x] Sign in with Bluesky (or another AT Protocol account)
- [x] Friends (mutual follows), chat (Bluesky DMs) and game invites
- [x] Discussion boards per game, built on Bluesky posts
- [ ] Board moderation (pinned and locked threads)
- [ ] Presence, group chats and lobbies

### Steam Deck and couch play
- [ ] Controller navigation (in progress)
- [ ] Add to Steam, so games show up in Steam and Game Mode (in progress; all OSes)
- [ ] Couch mode with Portside's own on-screen keyboard (all OSes)
- [ ] Library on the SD card (Steam Deck)
- [ ] Steam's on-screen keyboard (Steam Deck)
- [ ] Run Windows-only ports through Proton (Linux)
- [ ] Flathub package

### Platforms and releases
- [x] macOS, Windows and Linux builds on the releases page
- [ ] Signed builds (macOS notarization, Windows code signing)
- [ ] Android

## Run it from source

Needs Node 22, pnpm 10 and Rust via [rustup](https://rustup.rs).

```sh
cd client
pnpm install
pnpm tauri dev
```

More in [`client/README.md`](client/README.md).

## Contributing

- Bugs and ideas: open an issue.
- **Missing a game, or a port that should be installable?** Open an issue with the port's
  repository link. Install manifests and the catalog are maintained separately and published
  to the catalog site.
- Pull requests: one change per PR, with tests. CI runs `cargo fmt`, `clippy -D warnings`,
  the Rust tests and the UI checks. Conventions are in `CLAUDE.md` and `client/CLAUDE.md`.
- Never add game files, ROMs, BIOS files or links to them (`docs/legal/policy.md`).

## License

GPL-3.0-only. See `LICENSE`.
