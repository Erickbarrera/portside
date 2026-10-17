# Publishing the catalog and the website

`.github/workflows/catalog.yml` builds the static catalog (`catalog/`, from `manifests/` and
upstream GitHub releases) and the website (`web/`), and deploys both to this repository's
GitHub Pages: the site at `https://erickbarrera.github.io/portside/`, the catalog under
`v1/` (`docs/specs/static-catalog.md`).

## When it runs
- Once a day (06:17 UTC): rebuilds the catalog, and deploys only when it changed.
- On a push to `main` that touches `catalog/`, `manifests/`, `web/` or the workflow, and from
  Actions → catalog → Run workflow: always deploys.
- On a pull request: checks only (lint, types, tests, manifests, generated files, site build).

## Settings (once)
1. Settings → Pages → Build and deployment → Source: **GitHub Actions**.
2. Settings → Secrets and variables → Actions → Secrets:
   - `STEAMGRIDDB_API_KEY` (optional): covers for games without a picture
     (`catalog/src/steamgriddb.ts`). Without it the build keeps the published covers.
3. Variables (all optional): `SITE_URL` and `SITE_BASE` for a custom domain
   (`https://<domain>` and `/`) [TBD: custom domain]; `PORTSIDE_CATALOG_URL` if the catalog
   moves.

## The old address
Until 2026-10-17 the catalog was published at `https://erickbarrera.github.io/portside-catalog`.
It is no longer updated: app releases up to 0.2.0 keep reading the last catalog there until
they are updated. The first build here reads its releases and covers from that copy.
