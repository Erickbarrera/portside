# Portside app — instructions for Claude Code

This is the public repository of the Portside desktop app. Read `client/CLAUDE.md` before
working in `client/`.

## What's here
- `client/`: the app (Tauri 2, Rust workspace + Svelte UI).
- `docs/specs/`: the contracts the app builds against (static catalog, manifest format,
  boards, friends and chat, lexicons, save sync). They are copies: the source is the private
  Portside planning repository, which also builds the catalog and the website. Change a spec
  there first, then copy it here in the same change as the app code that follows it.
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
- pnpm workspace and Cargo workspace both rooted in `client/`. Node 22, TypeScript strict, ESM.
- Biome for TS/JS/JSON; `rustfmt` + `clippy -D warnings` for Rust.
- Commits: Conventional Commits (`feat(client): ...`, `fix(client): ...`, `docs: ...`).
- `main` is protected (`.github/rulesets/main.json`): PRs only, checks `rust` and `ui` must pass.
- Releases: push a `client-vX.Y.Z` tag matching the app version
  (`client/docs/build-release.md`).
