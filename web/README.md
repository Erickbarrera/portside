# Portside web

Static Astro site, published at https://erickbarrera.github.io/portside/
[TBD: custom domain]: landing page and catalog today; downloads, help, legal and OAuth client
metadata are planned (see `CLAUDE.md` and `docs/`). Published with the catalog on GitHub
Pages (ADR-0018).

## Run it locally
```sh
cd web
pnpm install
pnpm dev:mock      # synthetic catalog on :4010 + astro dev on :4321, exposed on your network
pnpm dev           # against the published catalog
```
Open http://localhost:4321 (home) and http://localhost:4321/catalog. From a phone or another
computer on the same network, use the Network URL Astro prints.

The catalog page reads the static Portside catalog (`../docs/specs/static-catalog.md`): the
first 50 ports are rendered at build time, and the browser loads the whole snapshot once for
search and "Load more". `/catalog?q=...` opens with a search. `.env.example` lists the
variables that point it at another copy.

## Deploy
Nothing to set up beyond the catalog's publishing steps (`../docs/publishing.md`).
The `catalog` workflow builds this site with the catalog and deploys both to this
repository's GitHub Pages:
- A push to `main` that touches `web/` publishes the site. Hourly runs republish only when the
  catalog changed, which also refreshes the first page baked into the HTML.
- Catalog updates need no publish of the site: the browser reads the live catalog.
- To check the Pages path locally: `PORTSIDE_SITE_BASE=/portside pnpm build && pnpm preview`.
- Custom domain [TBD]: set the repository variables `SITE_URL` (`https://<domain>`) and
  `SITE_BASE` (`/`), and the domain in this repository's Pages settings.
