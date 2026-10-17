# Portside web — instructions for Claude Code

Public website at `https://erickbarrera.github.io/portside/` [TBD: custom domain]: landing page, downloads, user help, legal
pages, and the OAuth client metadata file. Read the root `CLAUDE.md` first.

## Stack
- Astro 5 (static output), TypeScript, content collections for help/docs pages (Markdown).
- Styling: CSS variables shared with the client (tokens in `../client/docs/ui.md`);
  no CSS framework. Fonts self-hosted (IBM Plex, OFL).
- Islands only where needed (Svelte for the download picker).
- Hosting: GitHub Pages of this repository, next to the catalog (ADR-0018). The `catalog`
  workflow (`../.github/workflows/catalog.yml`) builds the site and the catalog together and
  deploys both in one Pages deployment. The site never depends on the
  desktop app or a Portside server (ADR-0015).
- The site lives under a path (`/portside/`): Astro's `base` comes from
  `PORTSIDE_SITE_BASE` (local default `/`). Write every internal link and asset path with
  `href()` from `src/lib/paths.ts`, never a bare `"/..."`.
- No response headers on GitHub Pages: put policies in `<meta>` tags (`Base.astro`).

## Planned layout
```
web/
  astro.config.mjs
  public/
    oauth/client-metadata.json   # AT Protocol OAuth public client metadata (planned, Milestone 3)
    .well-known/                 # e.g. atproto-did for the root handle, security.txt
    fonts/
  src/
    pages/ index.astro, download.astro, how-it-works.astro, faq.astro,
           privacy.astro, terms.astro, content-policy.astro, help/[...slug].astro
    components/ Hero, FeatureGrid, PlatformBadges, DownloadPicker (Svelte island), Footer
    content/help/*.md
    lib/releases.ts              # latest app release from GitHub (build time + browser)
  docs/
```

## Commands
Run from `web/`: `pnpm dev:mock` (synthetic catalog on :4010 + dev server on the local network
via `--host`) · `pnpm dev` (against the published catalog) · `pnpm build` · `pnpm check`.
`PORTSIDE_SITE_BASE=/portside pnpm build && pnpm preview` serves the site under the
path it has on GitHub Pages. `pnpm screenshots` recaptures the app screenshots in
`src/assets/screenshots/` from the client UI's mock mode with the real published catalog swapped
in (start `pnpm --filter @portside/ui dev` in `../client` first; Playwright needs a Chromium,
`pnpm exec playwright install chromium`; `PORTSIDE_CATALOG_DIR` reads a locally built catalog,
`../catalog/dist`, instead of the published URL). CI runs `pnpm check` and `pnpm build` in the `catalog` workflow.

## Catalog data
- The catalog is the static one from `docs/specs/static-catalog.md`, published to GitHub
  Pages of this repository. `src/lib/catalog.ts` holds its types (hand-copied from
  `catalog/src/schema/catalog.ts`; keep them in step), the URL and the search.
- URL: `PUBLIC_PORTSIDE_CATALOG_URL` (browser and build), `PORTSIDE_CATALOG_URL` (build only,
  overrides the first). Default `https://erickbarrera.github.io/portside`. See
  `.env.example`.
- The catalog page renders the first 50 ports by stars at build time. In the browser it
  downloads `v1/snapshot.json` once and does search (`?q=` in the URL) and "Load more"
  locally with `searchPorts`. When the live snapshot's `rev` differs from the build's, the
  list re-renders, so the site never needs a redeploy for catalog changes. An unreachable
  catalog at build time logs a warning and the browser tries again.
- `src/lib/port-card.ts` renders a port row as an HTML string for both the build and the
  browser, so the row styles live in `src/styles/global.css`, not scoped.
- Each row shows the port's `screenshot_url`, the same picture the desktop app shows, linked
  from where it is hosted (never fetched, copied or re-hosted by the site; policy rule 6).
  Without one, or when it fails to load, the row shows the app's generated gradient cover
  (`src/lib/cover.ts`, a copy of `../client/ui/src/lib/cover.ts`; keep
  them in step).
- `scripts/mock-catalog.mjs` serves synthetic rows (`scripts/fixtures.mjs`) as a static
  catalog; `dev:mock` proxies it at `/mock-catalog` so phones on the LAN work. Keep fixtures
  synthetic (150 numbered "Sample Game" rows exist so paging can be tried).

## Rules
- No game box art, logos or screenshots unless cleared (`../docs/legal/policy.md`). Use
  abstract art and Portside UI screenshots. App screenshots show the real catalog, game
  covers included (Erick, 2026-10-15; the website exception in policy rule 6): recapture them
  when a cover is turned off or taken down. People and chats in them stay made-up examples.
  They go through
  `astro:assets` (`<Picture>`, AVIF with WebP fallback), so commit the PNG sources only.
- Never link to ROM/asset sites. Copy says "bring your own game files".
- Downloads (the home page's button and `#download` section): `src/lib/releases.ts` reads the
  newest published release of this repository (`Erickbarrera/portside`)
  (pre-releases included) at build time and again in the browser, and maps its files to
  macOS, Windows and Linux. The button detects the visitor's OS and links straight to that
  file, so the download starts without leaving the site; the section lists every system,
  detected one first. `src/lib/download-html.ts` renders it for build and browser alike.
  Later: Flathub, F-Droid, Obtainium.
- Performance: no JS on pages that don't need it; Lighthouse ≥ 95 all categories.
- Accessibility: semantic HTML, 4.5:1 contrast, keyboard nav.
- Content for pages is in `docs/content-plan.md`; keep copy plain and specific.
