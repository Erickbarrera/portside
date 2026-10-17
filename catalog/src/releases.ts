import type { Release, ReleaseAsset } from "./schema/catalog.js";
/**
 * Upstream releases for ports with a manifest: read from the GitHub REST API at build time and
 * matched to the manifest's builds. Spec: docs/specs/static-catalog.md ("Releases").
 *
 * Selection follows the manifest's `source.channel`, the same way the desktop app picks a
 * release: `stable` skips drafts and prereleases, `prerelease` takes any non-draft, and
 * `tag_pattern` takes non-drafts whose tag matches the pattern.
 */
import type { Manifest } from "./schema/manifest.js";

/** Releases kept per port (newest first). */
export const RELEASES_PER_PORT = 10;

interface GhAsset {
  name: string;
  browser_download_url: string;
  size: number;
  digest?: string | null;
}

interface GhRelease {
  id: number;
  tag_name: string;
  name: string | null;
  draft: boolean;
  prerelease: boolean;
  published_at: string | null;
  html_url: string;
  assets: GhAsset[];
}

export interface GithubOptions {
  /** A token raises the rate limit from 60 to 1,000+ requests an hour. */
  token?: string | undefined;
  apiUrl?: string;
  fetch?: typeof fetch;
}

/** `*` and `?` wildcards and `[...]` classes, matched against the whole file name. */
export function globToRegExp(glob: string): RegExp {
  let re = "";
  for (let i = 0; i < glob.length; i++) {
    const c = glob[i] as string;
    if (c === "*") re += ".*";
    else if (c === "?") re += ".";
    else if (c === "[") {
      const end = glob.indexOf("]", i + 1);
      if (end === -1) re += "\\[";
      else {
        const body = glob.slice(i + 1, end).replace(/\\/g, "\\\\");
        re += `[${body.startsWith("!") ? `^${body.slice(1)}` : body}]`;
        i = end;
      }
    } else re += c.replace(/[.+^${}()|\\/]/g, "\\$&");
  }
  return new RegExp(`^${re}$`);
}

function selects(manifest: Manifest, r: GhRelease): boolean {
  if (r.draft || !r.published_at) return false;
  switch (manifest.source.channel ?? "stable") {
    case "stable":
      return !r.prerelease;
    case "prerelease":
      return true;
    case "tag_pattern": {
      const pattern = manifest.source.tag_pattern;
      return pattern ? new RegExp(pattern).test(r.tag_name) : false;
    }
  }
}

/** One release in the catalog's shape. Builds whose glob doesn't match exactly one file are left out. */
export function toRelease(portId: string, manifest: Manifest, r: GhRelease): Release {
  const assets: ReleaseAsset[] = [];
  for (const b of manifest.builds) {
    const re = globToRegExp(b.asset);
    const hits = r.assets.filter((a) => re.test(a.name));
    if (hits.length !== 1) continue;
    const a = hits[0] as GhAsset;
    const sha256 = a.digest?.startsWith("sha256:") ? a.digest.slice("sha256:".length) : null;
    assets.push({
      os: b.os,
      arch: b.arch,
      name: a.name,
      url: a.browser_download_url,
      size: a.size,
      sha256,
    });
  }
  return {
    id: String(r.id),
    port_id: portId,
    tag: r.tag_name,
    name: r.name || null,
    published_at: new Date(r.published_at as string).toISOString(),
    prerelease: r.prerelease,
    revoked: false,
    html_url: r.html_url,
    assets,
  };
}

/**
 * The one release a `url_list` manifest describes (ADR-0024): no network, every build has its
 * own url, size and sha256. Spec: static-catalog.md ("Releases").
 */
export function urlListRelease(manifest: Manifest): Release {
  const src = manifest.source;
  const version = src.version as string;
  const released = src.released as string;
  const day = /^\d{4}-\d{2}-\d{2}$/.test(released) ? `${released}T00:00:00Z` : released;
  return {
    id: `url_list:${version}`,
    port_id: manifest.id,
    tag: version,
    name: version,
    published_at: new Date(day).toISOString(),
    prerelease: false,
    revoked: false,
    html_url: src.release_url as string,
    assets: manifest.builds.map((b) => ({
      os: b.os,
      arch: b.arch,
      name: b.asset,
      url: b.url as string,
      size: b.size as number,
      sha256: b.sha256 ?? null,
    })),
  };
}

/** Newest releases the manifest selects. Throws on HTTP errors (the caller decides what to keep). */
export async function fetchReleases(
  manifest: Manifest,
  opts: GithubOptions = {},
): Promise<Release[]> {
  if (manifest.source.type === "url_list") return [urlListRelease(manifest)];
  const repo = manifest.source.repo;
  if (manifest.source.type !== "github_releases" || !repo) return [];
  const api = opts.apiUrl ?? "https://api.github.com";
  const headers: Record<string, string> = {
    accept: "application/vnd.github+json",
    "x-github-api-version": "2022-11-28",
    "user-agent": "portside-catalog",
  };
  if (opts.token) headers.authorization = `Bearer ${opts.token}`;
  const res = await (opts.fetch ?? fetch)(`${api}/repos/${repo}/releases?per_page=30`, {
    headers,
    signal: AbortSignal.timeout(30_000),
  });
  if (!res.ok) throw new Error(`${repo}: GitHub returned HTTP ${res.status}`);
  const list = (await res.json()) as GhRelease[];
  return list
    .filter((r) => selects(manifest, r))
    .sort((a, b) => Date.parse(b.published_at ?? "") - Date.parse(a.published_at ?? ""))
    .slice(0, RELEASES_PER_PORT)
    .map((r) => toRelease(manifest.id, manifest, r));
}
