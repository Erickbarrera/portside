/**
 * pnpm catalog <command>
 *
 *   build      Write the static catalog to --out (default: dist). Options:
 *                --upstream <url|path>   games.json (default: UPSTREAM_GAMES_URL or awesome-decomp-games)
 *                --manifests <dir>       manifests folder (default: ../manifests)
 *                --previous <url>        the published catalog's base URL (default: CATALOG_PREVIOUS_URL),
 *                                        to keep rev/updated_at and to fall back on its releases
 *                --no-releases           skip GitHub release and preview-image lookups (offline builds)
 *              GITHUB_TOKEN raises the GitHub rate limit and is needed for repo preview images.
 *              STEAMGRIDDB_API_KEY looks up SteamGridDB covers for games with no other image
 *              (overrides: <manifests>/covers.yaml). In GitHub Actions, writes
 *              `changed=true|false` to GITHUB_OUTPUT.
 *   validate   Check every manifest; exit 1 if any is invalid.
 *
 * Spec: docs/specs/static-catalog.md.
 */
import { appendFile, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { type CatalogIndex, buildCatalog } from "./build.js";
import { type LoadedManifest, loadManifests } from "./manifests.js";
import { fetchPreviews } from "./previews.js";
import { fetchReleases } from "./releases.js";
import type { CatalogSnapshot, Release } from "./schema/catalog.js";
import {
  type CoverCache,
  type CoverOverrides,
  parseOverrides,
  readCache,
  updateCovers,
} from "./steamgriddb.js";
import { gameTitle, parseUpstream, portColumns } from "./upstream.js";

export const DEFAULT_UPSTREAM =
  "https://raw.githubusercontent.com/SubmitGame/awesome-decomp-games/main/data/games.json";
const DEFAULT_MANIFESTS = resolve(import.meta.dirname, "../../manifests");

async function readJson(source: string): Promise<unknown> {
  if (!/^https?:\/\//.test(source)) return JSON.parse(await readFile(source, "utf8"));
  const res = await fetch(source, {
    headers: { "user-agent": "portside-catalog" },
    signal: AbortSignal.timeout(60_000),
  });
  if (!res.ok) throw new Error(`HTTP ${res.status} from ${source}`);
  return res.json();
}

/** A file from the published catalog, or null when there is none (first build, offline). */
async function previousFile<T>(base: string | undefined, path: string): Promise<T | null> {
  if (!base) return null;
  try {
    return (await readJson(`${base.replace(/\/$/, "")}/${path}`)) as T;
  } catch {
    return null;
  }
}

async function validManifests(dir: string): Promise<LoadedManifest[]> {
  const { valid, invalid } = await loadManifests(dir);
  for (const v of valid) {
    for (const w of v.warnings) console.warn(`warning: ${v.file}${w.path}: ${w.message}`);
  }
  if (invalid.length > 0) {
    for (const i of invalid) {
      for (const e of i.errors) console.error(`error: ${i.file}${e.path}: ${e.message}`);
    }
    throw new Error(`${invalid.length} invalid manifest(s)`);
  }
  return valid;
}

async function releasesFor(
  manifests: LoadedManifest[],
  previous: string | undefined,
): Promise<Map<string, Release[]>> {
  const out = new Map<string, Release[]>();
  const token = process.env.GITHUB_TOKEN || undefined;
  const queue = [...manifests];
  const worker = async () => {
    for (let m = queue.shift(); m; m = queue.shift()) {
      const id = m.manifest.id;
      try {
        const releases = await fetchReleases(m.manifest, { token });
        out.set(id, releases);
        const newest = releases[0];
        for (const b of m.manifest.builds) {
          if (newest && !newest.assets.some((a) => a.os === b.os && a.arch === b.arch)) {
            console.warn(
              `warning: ${m.file}: ${b.os}/${b.arch} asset "${b.asset}" matches no single file in ${newest.tag}`,
            );
          }
        }
      } catch (err) {
        // Keep what's published rather than dropping a port's releases on a transient error.
        const kept = await previousFile<Release[]>(previous, `v1/ports/${id}/releases.json`);
        console.warn(
          `warning: ${(err as Error).message}; ${kept ? "keeping the published releases" : "no releases"}`,
        );
        if (kept) out.set(id, kept);
      }
    }
  };
  await Promise.all(Array.from({ length: 6 }, worker));
  return out;
}

/** Repo preview images, or null (keep the published ones) without a token or on an error. */
async function previewsFor(upstream: unknown): Promise<Map<string, string> | null> {
  const token = process.env.GITHUB_TOKEN;
  if (!token) {
    console.warn("warning: no GITHUB_TOKEN; keeping the published repo preview images");
    return null;
  }
  const list =
    (Array.isArray(upstream) ? upstream : (upstream as { games?: unknown })?.games) ?? [];
  const repos = (list as { repo?: unknown; screenshot?: unknown }[])
    .filter((e) => typeof e?.repo === "string" && !e.screenshot)
    .map((e) => e.repo as string);
  try {
    return await fetchPreviews(repos, { token });
  } catch (err) {
    console.warn(`warning: ${(err as Error).message}; keeping the published repo preview images`);
    return null;
  }
}

async function readOverrides(manifestsDir: string): Promise<CoverOverrides> {
  try {
    return parseOverrides(await readFile(join(manifestsDir, "covers.yaml"), "utf8"));
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return new Map();
    throw err;
  }
}

/**
 * SteamGridDB covers for games that have a port with no list screenshot and no repo preview,
 * and for every game in covers.yaml (an entry replaces the game's own images).
 * Without a key, or without fresh preview images to tell which games need one, the published
 * cache is kept as is.
 */
async function coversFor(
  upstream: unknown,
  previews: Map<string, string> | null,
  previous: CoverCache | null,
  overrides: CoverOverrides,
): Promise<CoverCache | null> {
  const key = process.env.STEAMGRIDDB_API_KEY;
  if (!key || !previews) {
    if (!key)
      console.warn("warning: no STEAMGRIDDB_API_KEY; keeping the published SteamGridDB covers");
    return previous;
  }
  const best = new Map<string, { title: string; stars: number }>();
  const wanted = new Map<string, string>();
  for (const e of parseUpstream(upstream).entries) {
    const stars = e.stars ?? -1;
    const b = best.get(e.game_key);
    if (!b || stars > b.stars) best.set(e.game_key, { title: gameTitle(e.title), stars });
    const cols = portColumns(e);
    // A covers.yaml entry replaces the game's own images, so it is looked up regardless.
    if (
      overrides.has(e.game_key) ||
      (!cols.screenshot_url && !previews.has(cols.repo?.toLowerCase() ?? ""))
    ) {
      wanted.set(e.game_key, "");
    }
  }
  for (const k of wanted.keys()) wanted.set(k, best.get(k)?.title ?? k);
  const today = new Date().toISOString().slice(0, 10);
  const cache = await updateCovers({
    wanted,
    cache: previous ?? readCache(null),
    overrides,
    opts: { key },
    today,
    log: (msg) => console.warn(`warning: ${msg}`),
  });
  const games = Object.values(cache.games);
  console.log(
    `SteamGridDB: ${games.filter((g) => g.url).length} covers for ${wanted.size} games without an image ` +
      `(${games.filter((g) => g.checked === today).length} checked today)`,
  );
  return cache;
}

async function build(args: string[]): Promise<void> {
  const { values } = parseArgs({
    args,
    options: {
      out: { type: "string", default: "dist" },
      upstream: { type: "string" },
      manifests: { type: "string" },
      previous: { type: "string" },
      "no-releases": { type: "boolean", default: false },
    },
  });
  const upstreamSource = values.upstream ?? process.env.UPSTREAM_GAMES_URL ?? DEFAULT_UPSTREAM;
  const previousUrl = values.previous ?? (process.env.CATALOG_PREVIOUS_URL || undefined);
  const manifestsDir = values.manifests ?? DEFAULT_MANIFESTS;
  const manifests = await validManifests(manifestsDir);
  const overrides = await readOverrides(manifestsDir);
  const [upstream, index, snapshot, coverCache] = await Promise.all([
    readJson(upstreamSource),
    previousFile<CatalogIndex>(previousUrl, "v1/index.json"),
    previousFile<CatalogSnapshot>(previousUrl, "v1/snapshot.json"),
    previousFile<unknown>(previousUrl, "build/steamgriddb.json"),
  ]);
  const previousCovers = coverCache ? readCache(coverCache) : null;
  const [releases, previews] = values["no-releases"]
    ? [new Map<string, Release[]>(), null]
    : await Promise.all([releasesFor(manifests, previousUrl), previewsFor(upstream)]);
  const covers = values["no-releases"]
    ? previousCovers
    : await coversFor(upstream, previews, previousCovers, overrides);

  const result = buildCatalog({
    upstream,
    manifests,
    releases,
    previews,
    covers,
    coverOverrides: new Set(overrides.keys()),
    previous: { index, snapshot },
    now: new Date(),
  });
  for (const w of result.warnings) console.warn(`warning: ${w}`);

  const out = resolve(values.out);
  await rm(out, { recursive: true, force: true });
  for (const [path, body] of result.files) {
    const file = join(out, path);
    await mkdir(dirname(file), { recursive: true });
    await writeFile(file, body);
  }
  console.log(
    `catalog rev ${result.index.rev}: ${result.index.port_count} ports, ${manifests.length} manifests, ` +
      `${[...releases.values()].reduce((n, r) => n + r.length, 0)} releases, ` +
      `${result.invalidEntries} upstream entries skipped; ${result.changed ? "changed" : "unchanged"} → ${out}`,
  );
  if (process.env.GITHUB_OUTPUT) {
    await appendFile(process.env.GITHUB_OUTPUT, `changed=${result.changed}\n`);
  }
}

async function main(): Promise<void> {
  const [command, ...rest] = process.argv.slice(2);
  switch (command) {
    case "build":
      return build(rest);
    case "validate": {
      const dir = rest[0] ?? DEFAULT_MANIFESTS;
      const valid = await validManifests(dir);
      console.log(`${valid.length} manifest(s) valid`);
      return;
    }
    default:
      console.error("usage: pnpm catalog build [options] | pnpm catalog validate [dir]");
      process.exitCode = 2;
  }
}

main().catch((err: unknown) => {
  console.error(`error: ${err instanceof Error ? err.message : String(err)}`);
  process.exitCode = 1;
});
