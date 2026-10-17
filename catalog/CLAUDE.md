# Portside catalog builder — instructions for Claude Code

Read the root `CLAUDE.md` first. What this builds and why: `README.md`, ADR-0015.
Contract: `../docs/specs/static-catalog.md` (files) and `../docs/specs/catalog.md`
(upstream mapping, playable status), `../docs/specs/manifest-format.md` (manifests).

## Layout
```
src/
  schema/      TypeBox schemas: enums, manifest (validateManifest), catalog file shapes
  upstream.ts  games.json → port fields, seed status (pure)
  manifests.ts load + validate ../manifests/ports/*.yaml
  releases.ts  GitHub releases → Release[] per manifest (channel rules, asset globs)
  previews.ts  repos' custom GitHub preview images (GraphQL), the fallback cover
  steamgriddb.ts  SteamGridDB covers per game (last fallback) and their lookup cache
  build.ts     everything → files (pure: no network, no disk)
  cli.ts       `build` and `validate`; network and disk live here
scripts/       gen-schema.ts (manifest JSON Schema), write-contract.ts (OpenAPI document)
test/          vitest; fixtures are synthetic (invented ports, no real game names or files)
```

## Rules
- The file shapes are a contract with the desktop app and the website. Change
  `docs/specs/static-catalog.md` first, then `src/schema/catalog.ts`, run
  `pnpm contract:write`, and update `web/src/lib/catalog.ts` and
  `client/crates/portside-api/src/models.rs` (its contract test reads the document) in the
  same PR.
- Keep `build.ts` pure and deterministic for the same input: `rev` and `updated_at` must not
  change when the content doesn't, or every client re-downloads the catalog after every build.
- An invalid manifest must fail the build; a GitHub error for one port must not (keep the
  published releases for it).
- No game assets or links to them (`../docs/legal/policy.md`). Screenshots are only linked,
  never copied, and only from the sources in `../docs/specs/static-catalog.md`
  ("Screenshots"; `src/previews.ts` reads repo preview images, `src/steamgriddb.ts`
  SteamGridDB covers with `STEAMGRIDDB_API_KEY`; fix wrong pictures, from any source, in
  `../manifests/covers.yaml`).
- Commands before pushing: `pnpm lint && pnpm typecheck && pnpm test`, and
  `pnpm schema:gen && pnpm contract:write` with no diff.
