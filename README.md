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
