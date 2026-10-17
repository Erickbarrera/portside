/**
 * Covers from SteamGridDB for games with no list screenshot and no repo preview image, or whose
 * images manifests/covers.yaml replaces (docs/specs/static-catalog.md, "Screenshots"; ADR-0020).
 * One cover per game, matched by title, linked from SteamGridDB's CDN.
 *
 * Lookups are remembered in a cache published with the catalog (`build/steamgriddb.json`), so
 * each build only looks up games it hasn't checked recently, a limited number per run.
 */
import { parse as parseYaml } from "yaml";

export const SGDB_API = "https://www.steamgriddb.com/api/v2";
/** Landscape grids, closest to the app's 16:9 covers. */
const GRID_QUERY = "dimensions=920x430,460x215&types=static&nsfw=false&humor=false";
/** Games looked up per build (two requests each). */
export const LOOKUP_BUDGET = 150;
/** Days before a game is looked up again: misses sooner, since art gets added. */
export const RECHECK_HIT_DAYS = 30;
export const RECHECK_MISS_DAYS = 7;

export interface CoverEntry {
  /** SteamGridDB game id, or null when no game matched. */
  id: number | null;
  url: string | null;
  /** Day of the lookup, YYYY-MM-DD. */
  checked: string;
}

export interface CoverCache {
  schema: 1;
  /** By game_key. */
  games: Record<string, CoverEntry>;
}

/** game_key → SteamGridDB game id, or "none" for no cover. From manifests/covers.yaml. */
export type CoverOverrides = Map<string, number | "none">;

export interface SgdbOptions {
  key: string;
  apiUrl?: string;
  fetchFn?: typeof fetch;
}

/** game_keys usable as object keys (and file names); others get no cover. */
const SAFE_KEY = /^[a-z0-9][a-z0-9._-]*$/i;

export function emptyCache(): CoverCache {
  return { schema: 1, games: {} };
}

/** The published cache, or an empty one when it is missing or unreadable. */
export function readCache(json: unknown): CoverCache {
  const games = (json as { schema?: unknown; games?: unknown } | null)?.games;
  if ((json as { schema?: unknown } | null)?.schema !== 1 || !games || typeof games !== "object") {
    return emptyCache();
  }
  const out = emptyCache();
  for (const [key, e] of Object.entries(games as Record<string, Partial<CoverEntry>>)) {
    if (
      SAFE_KEY.test(key) &&
      e &&
      (e.id === null || Number.isInteger(e.id)) &&
      (e.url === null || (typeof e.url === "string" && isCdnUrl(e.url))) &&
      typeof e.checked === "string" &&
      /^\d{4}-\d{2}-\d{2}$/.test(e.checked)
    ) {
      out.games[key] = {
        id: e.id as number | null,
        url: e.url as string | null,
        checked: e.checked,
      };
    }
  }
  return out;
}

/** manifests/covers.yaml: `game_key: <SteamGridDB game id>` or `game_key: none`. */
export function parseOverrides(text: string): CoverOverrides {
  const doc: unknown = parseYaml(text) ?? {};
  if (typeof doc !== "object" || Array.isArray(doc)) {
    throw new Error("covers.yaml: expected game_key: id | none");
  }
  const out: CoverOverrides = new Map();
  for (const [key, v] of Object.entries(doc as Record<string, unknown>)) {
    if (v === "none" || (Number.isInteger(v) && (v as number) > 0)) {
      out.set(key, v as number | "none");
    } else {
      throw new Error(`covers.yaml: ${key}: expected a SteamGridDB game id or none`);
    }
  }
  return out;
}

function isCdnUrl(url: string): boolean {
  try {
    const u = new URL(url);
    return (
      u.protocol === "https:" &&
      (u.hostname === "steamgriddb.com" || u.hostname.endsWith(".steamgriddb.com"))
    );
  } catch {
    return false;
  }
}

/** Title for matching: lowercase letters and digits only, "&" read as "and". */
export function normalizeTitle(s: string): string {
  return s
    .normalize("NFKD")
    .toLowerCase()
    .replace(/&/g, "and")
    .replace(/[^a-z0-9]/g, "");
}

/** The search result whose name is the title, ignoring case and punctuation; else null. */
export function pickGame(results: { id: number; name: string }[], title: string): number | null {
  const want = normalizeTitle(title);
  if (!want) return null;
  return results.find((r) => normalizeTitle(r.name) === want)?.id ?? null;
}

/**
 * The best-scored grid on SteamGridDB's CDN; `keep` (the current cover) wins while it is still
 * listed, so covers don't flip when scores move.
 */
export function pickGrid(
  grids: { id: number; score?: number; url: string }[],
  keep?: string | null,
): string | null {
  const ok = grids.filter((g) => isCdnUrl(g.url));
  if (keep && ok.some((g) => g.url === keep)) return keep;
  ok.sort((a, b) => (b.score ?? 0) - (a.score ?? 0) || a.id - b.id);
  return ok[0]?.url ?? null;
}

async function get<T>(path: string, opts: SgdbOptions): Promise<T[]> {
  const doFetch = opts.fetchFn ?? fetch;
  const res = await doFetch(`${opts.apiUrl ?? SGDB_API}${path}`, {
    headers: { authorization: `Bearer ${opts.key}`, "user-agent": "portside-catalog" },
    signal: AbortSignal.timeout(30_000),
  });
  // An unknown game id answers 404; that's a miss, not a failure.
  if (res.status === 404) return [];
  if (!res.ok) throw new Error(`SteamGridDB: HTTP ${res.status}`);
  const body = (await res.json()) as { success?: boolean; data?: T[] };
  if (!body.success || !Array.isArray(body.data)) throw new Error("SteamGridDB: unexpected answer");
  return body.data;
}

/** Looks one game up. `id` skips the search (an override or an earlier match). */
export async function lookupCover(
  title: string,
  opts: SgdbOptions,
  known: { id?: number | null; url?: string | null } = {},
): Promise<{ id: number | null; url: string | null }> {
  let id = known.id ?? null;
  if (id === null) {
    const results = await get<{ id: number; name: string }>(
      `/search/autocomplete/${encodeURIComponent(title)}`,
      opts,
    );
    id = pickGame(results, title);
    if (id === null) return { id: null, url: null };
  }
  const grids = await get<{ id: number; score?: number; url: string }>(
    `/grids/game/${id}?${GRID_QUERY}`,
    opts,
  );
  return { id, url: pickGrid(grids, known.url) };
}

function daysBetween(from: string, to: string): number {
  return (Date.parse(to) - Date.parse(from)) / 86_400_000;
}

/**
 * Brings the cache up to date for `wanted` (game_key → title): new or changed covers.yaml ids
 * first, then games never checked, then the stalest, at most `budget` lookups. Games not wanted any more are dropped. Stops looking
 * up at the first failed request (bad key, rate limit) and keeps what it has.
 */
export async function updateCovers(input: {
  wanted: Map<string, string>;
  cache: CoverCache;
  overrides: CoverOverrides;
  opts: SgdbOptions;
  today: string;
  budget?: number;
  log?: (msg: string) => void;
}): Promise<CoverCache> {
  const { wanted, cache, overrides, opts, today } = input;
  const out = emptyCache();
  const due: { key: string; overridden: boolean; age: number }[] = [];
  for (const key of [...wanted.keys()].filter((k) => SAFE_KEY.test(k)).sort()) {
    const o = overrides.get(key);
    const prev = cache.games[key];
    if (o === "none") {
      out.games[key] = { id: null, url: null, checked: prev?.checked ?? today };
      continue;
    }
    if (prev) out.games[key] = prev;
    const overridden = typeof o === "number" && prev?.id !== o;
    const age = prev ? daysBetween(prev.checked, today) : Number.POSITIVE_INFINITY;
    const stale = age >= (prev?.url ? RECHECK_HIT_DAYS : RECHECK_MISS_DAYS);
    if (!prev || overridden || stale) due.push({ key, overridden, age });
  }
  // A covers.yaml fix shows in the next build, not after the backlog of new games.
  due.sort(
    (a, b) =>
      Number(b.overridden) - Number(a.overridden) || b.age - a.age || (a.key < b.key ? -1 : 1),
  );
  for (const { key } of due.slice(0, input.budget ?? LOOKUP_BUDGET)) {
    const o = overrides.get(key);
    const prev = cache.games[key];
    const id = typeof o === "number" ? o : (prev?.id ?? null);
    try {
      const found = await lookupCover(wanted.get(key) as string, opts, {
        id,
        url: id === prev?.id ? prev?.url : null,
      });
      out.games[key] = { ...found, checked: today };
    } catch (err) {
      input.log?.(`${(err as Error).message}; keeping the cached covers`);
      break;
    }
  }
  return out;
}
