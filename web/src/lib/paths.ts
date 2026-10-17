// Links inside the site. The site may live under a path (`/portside/` on GitHub
// Pages, ADR-0018), so every internal link goes through `href`, never a bare "/...".
const base = import.meta.env.BASE_URL.replace(/\/+$/, "");

/** `href("/catalog")` → "/portside/catalog" on GitHub Pages, "/catalog" locally. */
export const href = (path: string): string => `${base}${path.startsWith("/") ? path : `/${path}`}`;
