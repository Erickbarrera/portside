# Publishing the catalog and the website

`.github/workflows/catalog.yml` builds the static catalog (`catalog/`, from `manifests/` and
upstream GitHub releases) and the website (`web/`), and deploys both to this repository's
GitHub Pages: the site at `https://erickbarrera.github.io/portside/`, the catalog under
`v1/` (`docs/specs/static-catalog.md`).

## When it runs
- Every hour: rebuilds the catalog, and deploys only when it changed.
- On a push to `main` that touches `catalog/`, `manifests/`, `web/` or the workflow, and from
  Actions → catalog → Run workflow: always deploys.
- On a pull request: checks only (lint, types, tests, manifests, generated files, site build).

## Settings (once)
1. Settings → Pages → Build and deployment → Source: **GitHub Actions**.
2. Settings → Secrets and variables → Actions → Secrets:
   - `STEAMGRIDDB_API_KEY` (optional): covers for games without a picture
     (`catalog/src/steamgriddb.ts`). Without it the build keeps the published covers.
   - `CATALOG_DEPLOY_KEY` (optional): the private half of a deploy key with write access on
     `Erickbarrera/portside-catalog`. With it, each deploy also copies the catalog to the old
     address below.
3. Variables (all optional): `SITE_URL` and `SITE_BASE` for a custom domain
   (`https://<domain>` and `/`) [TBD: custom domain]; `PORTSIDE_CATALOG_URL` if the catalog
   moves; `CATALOG_OLD_REPO` to copy to another repository than `Erickbarrera/portside-catalog`.

## The old address
App releases up to 0.2.0 read the catalog from `https://erickbarrera.github.io/portside-catalog`
(the `gh-pages` branch of `Erickbarrera/portside-catalog`), and their game invites link to the
website there. While `CATALOG_DEPLOY_KEY` is set, every deploy pushes the same `v1/` and
`build/` files there, plus an `index.html` and `404.html` that forward every other page to the
new site, keeping the path and query. Keep it until few people run those releases.
