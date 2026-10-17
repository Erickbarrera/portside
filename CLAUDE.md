# Portside — instructions for Claude Code

This is the public repository of Portside: the desktop app, the catalog it reads, and the
website. Read the area's `CLAUDE.md` before working in it.

## What's here
- `client/`: the app (Tauri 2, Rust workspace + Svelte UI) → `client/CLAUDE.md`.
- `catalog/`: the static catalog builder (TypeScript, runs in GitHub Actions) →
  `catalog/CLAUDE.md`. There is no hosted backend; don't add a server.
- `manifests/ports/`: which release file to install per OS → `manifests/README.md`.
- `web/`: the website (Astro) → `web/CLAUDE.md`.
- `.github/workflows/catalog.yml`: builds the catalog and the website every hour and deploys
  both to this repository's GitHub Pages, `https://erickbarrera.github.io/portside/`
  (catalog under `v1/`). It also copies the catalog to the old address,
  `https://erickbarrera.github.io/portside-catalog`, for app releases up to 0.2.0.
- `docs/specs/`: the contracts shared by the builder, the app and the website (static
  catalog, manifest format, boards, friends and chat, lexicons, save sync). This copy is the
  source; change the spec first, then the code that follows it, in the same PR.
- `docs/legal/policy.md`: what may never be added (game files, links to them).
- Architecture decision records (ADR-00xx in comments and docs) live in the private
  repository; the specs and `client/docs/` say what the app needs to know.

## Ground rules
1. **Never add game assets.** No ROMs, ISOs, BIOS files, extracted assets, or links to them,
   in code, tests, fixtures or docs. Test fixtures use synthetic files.
2. **Specs before code.** A change to a contract in `docs/specs/` updates the spec first.
3. **Small, reviewable steps.** One feature per PR, tests with the code.
4. **No secrets in the repo.**
5. **Placeholders are explicit.** Unknown values are written as `[TBD: ...]`.

## Conventions
- pnpm and Cargo workspaces rooted in `client/`; `catalog/` and `web/` each have their own
  pnpm lockfile. Node 22, TypeScript strict, ESM.
- Biome for TS/JS/JSON; `rustfmt` + `clippy -D warnings` for Rust.
- Commits: Conventional Commits. Scopes: `client`, `catalog`, `manifests`, `web`, `specs`, `docs`.
- `main` is protected (`.github/rulesets/main.json`): PRs only, checks `rust` and `ui` must pass.
- Releases: push a `client-vX.Y.Z` tag matching the app version
  (`client/docs/build-release.md`).
