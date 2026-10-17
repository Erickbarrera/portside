# Portside catalog builder

Builds the static Portside catalog (`docs/specs/static-catalog.md`, ADR-0015) from:
- games.json from [awesome-decomp-games](https://github.com/SubmitGame/awesome-decomp-games) (CC0),
- the port manifests in `../manifests/ports/`,
- upstream GitHub releases of ports that have a manifest.

The output is plain JSON files (`v1/snapshot.json`, `v1/ports/{id}.json`, …).
`.github/workflows/catalog.yml` builds it once a day and deploys it to this repository's
GitHub Pages (setup in `../docs/publishing.md`).

## Commands
```sh
pnpm install
pnpm build                    # → dist/ (fetches games.json and GitHub releases)
pnpm build --no-releases      # offline-friendly: skip GitHub
pnpm build --upstream path/to/games.json --out /tmp/catalog
pnpm validate                 # check every manifest in ../manifests
pnpm test · pnpm typecheck · pnpm lint
pnpm schema:gen               # → schemas/manifest.v1.json (commit it)
pnpm contract:write           # → ../docs/specs/static-catalog/v1.json (commit it)
```
Environment: `GITHUB_TOKEN` (optional, raises the GitHub rate limit), `UPSTREAM_GAMES_URL`,
`CATALOG_PREVIOUS_URL` (the published catalog, to keep `rev` and `updated_at` stable).

To point the desktop app at a local build: serve `dist/` (for example `npx serve dist`) and
start the app with `PORTSIDE_CATALOG_URL=http://localhost:3000`.
