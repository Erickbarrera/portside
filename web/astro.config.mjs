import { fileURLToPath } from "node:url";
import { defineConfig } from "astro/config";

try {
  process.loadEnvFile(fileURLToPath(new URL(".env", import.meta.url))); // shell vars still win
} catch {}

// `pnpm dev:mock` points the browser at /mock-catalog on this site; this forwards it to the
// mock catalog server (scripts/mock-catalog.mjs), so the page works from other devices on
// the LAN without CORS setup. Unused otherwise.
const mockProxy = {
  "/mock-catalog": {
    target: "http://localhost:4010",
    changeOrigin: true,
    rewrite: (path) => path.replace(/^\/mock-catalog/, ""),
  },
};

// Published on GitHub Pages of this repository, next to the catalog
// files (ADR-0018): https://erickbarrera.github.io/portside/. The publish workflow sets
// both variables; locally the site runs at the root (http://localhost:4321/).
// [TBD: custom domain] — then PORTSIDE_SITE_URL=https://<domain> and PORTSIDE_SITE_BASE=/.
export default defineConfig({
  site: process.env.PORTSIDE_SITE_URL || "https://erickbarrera.github.io",
  base: process.env.PORTSIDE_SITE_BASE || "/",
  output: "static",
  vite: {
    server: { proxy: mockProxy },
    preview: { proxy: mockProxy },
  },
});
