/**
 * Builds the static catalog from games.json, the manifests and upstream releases.
 * Pure: no network or disk access here (src/cli.ts does both). Spec: docs/specs/static-catalog.md.
 */
import { createHash } from "node:crypto";
import type { LoadedManifest } from "./manifests.js";
import type {
  CatalogSnapshot,
  Game,
  OsStatus,
  PortDetail,
  PortSummary,
  Release,
} from "./schema/catalog.js";
import { OS_VALUES, type PortType } from "./schema/enums.js";
import type { CoverCache } from "./steamgriddb.js";
import {
  type UpstreamEntry,
  gameTitle,
  originalPlatforms,
  parseUpstream,
  portColumns,
  portType,
  seedStatus,
} from "./upstream.js";

/** `v1/index.json`: what a build produced. */
export interface CatalogIndex {
  schema: 1;
  rev: number;
  generated_at: string;
  port_count: number;
  /** sha256 over every other file; equal hashes mean nothing to publish. */
  content_hash: string;
}

export interface BuildInput {
  upstream: unknown;
  manifests: LoadedManifest[];
  /** Releases per port id, newest first. Ports without an entry get none. */
  releases: Map<string, Release[]>;
  /**
   * Custom GitHub preview images by repo (lowercase), the cover when upstream has no
   * screenshot. Null or absent when they couldn't be fetched: the published covers are kept.
   */
  previews?: Map<string, string> | null;
  /**
   * SteamGridDB covers by game_key, the last fallback. Published as `build/steamgriddb.json`
   * so the next build only looks up what's new or stale. Absent: no cover from it.
   */
  covers?: CoverCache | null;
  /**
   * game_keys with a `manifests/covers.yaml` entry: their SteamGridDB cover (or none) replaces
   * the list screenshot and the repo preview image.
   */
  coverOverrides?: ReadonlySet<string>;
  /** The currently published catalog, to keep `rev` and `updated_at` when nothing changed. */
  previous?: { index: CatalogIndex | null; snapshot: CatalogSnapshot | null };
  now: Date;
}

export interface BuildOutput {
  /** Published path (relative to the site root) → file contents. */
  files: Map<string, string>;
  index: CatalogIndex;
  /** False when the content equals the previous build's. */
  changed: boolean;
  warnings: string[];
  invalidEntries: number;
}

/** Latest releases shown on a port's detail. */
const DETAIL_RELEASES = 5;
const SAFE_NAME = /^[a-z0-9][a-z0-9._-]*$/i;

const byStars = (a: PortSummary, b: PortSummary) =>
  (b.stars ?? -1) - (a.stars ?? -1) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);

function summaryHash(s: PortSummary): string {
  const { updated_at: _ignored, ...rest } = s;
  return createHash("sha256").update(JSON.stringify(rest)).digest("hex");
}

export function buildCatalog(input: BuildInput): BuildOutput {
  const warnings: string[] = [];
  const parsed = parseUpstream(input.upstream);
  let invalidEntries = parsed.invalid;
  const entries: UpstreamEntry[] = [];
  for (const e of parsed.entries) {
    if (portType(e.type)) entries.push(e);
    else invalidEntries++;
  }
  if (parsed.duplicateIds.length > 0) {
    warnings.push(
      `games.json repeats ids (kept the most-starred): ${parsed.duplicateIds.join(", ")}`,
    );
  }

  const manifests = new Map(input.manifests.map((m) => [m.manifest.id, m]));
  const ids = new Set(entries.map((e) => e.id));
  for (const id of manifests.keys()) {
    if (!ids.has(id))
      warnings.push(`manifests/ports/${id}.yaml: no port with this id in games.json`);
  }

  const prevSummaries = new Map(
    (input.previous?.snapshot?.ports ?? []).map((p) => [p.id, p] as const),
  );
  const nowIso = input.now.toISOString();
  const osOrder = (os: string) => OS_VALUES.indexOf(os as (typeof OS_VALUES)[number]);

  const summaries: PortSummary[] = [];
  const details = new Map<string, PortDetail>();
  for (const e of entries) {
    const cols = portColumns(e);
    const loaded = manifests.get(e.id);
    const m = loaded?.manifest;
    const os_status: OsStatus[] = m
      ? [...new Set(m.builds.map((b) => b.os))]
          .sort((a, b) => osOrder(a) - osOrder(b))
          .map((os) => ({ os, status: m.status, source: "manifest" as const }))
      : [];
    const summary: PortSummary = {
      id: cols.id,
      game_key: cols.game_key,
      // A manifest's display_name outranks the upstream title.
      title: m?.display_name ?? cols.title,
      type: cols.type as PortType,
      // A manifest's status outranks the upstream seed (docs/specs/catalog.md, "Merge model").
      status: m ? m.status : seedStatus(e),
      os_status,
      stars: cols.stars,
      archived: cols.archived,
      screenshot_url: input.coverOverrides?.has(cols.game_key)
        ? (input.covers?.games[cols.game_key]?.url ?? null)
        : (cols.screenshot_url ??
          (input.previews
            ? input.previews.get(cols.repo?.toLowerCase() ?? "")
            : prevSummaries.get(cols.id)?.screenshot_url) ??
          input.covers?.games[cols.game_key]?.url ??
          null),
      play_url: cols.play_url,
      updated_at: nowIso,
    };
    const prev = prevSummaries.get(summary.id);
    if (prev && summaryHash(prev) === summaryHash(summary)) summary.updated_at = prev.updated_at;
    summaries.push(summary);

    const releases = input.releases.get(e.id) ?? [];
    details.set(e.id, {
      ...summary,
      description: cols.description,
      repo: cols.repo,
      homepage: cols.homepage,
      license: cols.license,
      last_commit: cols.last_commit,
      upstream_score: cols.upstream_score,
      ratings: cols.ratings,
      video_url: cols.video_url,
      note: cols.note,
      status_notes: m?.status_notes ?? null,
      manifest: m ?? null,
      manifest_rev: loaded?.rev ?? null,
      verified: m?.maintainers_verified === true,
      latest_releases: releases.filter((r) => !r.revoked).slice(0, DETAIL_RELEASES),
    });
  }
  summaries.sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));

  // Games: title from the most-starred port, platforms from all of its ports.
  const gameEntries = new Map<string, { best: UpstreamEntry; platforms: string[] }>();
  for (const e of entries) {
    const g = gameEntries.get(e.game_key);
    if (!g) {
      gameEntries.set(e.game_key, { best: e, platforms: originalPlatforms(e.platform) });
      continue;
    }
    if ((e.stars ?? -1) > (g.best.stars ?? -1)) g.best = e;
    for (const p of originalPlatforms(e.platform))
      if (!g.platforms.includes(p)) g.platforms.push(p);
  }

  // Revision: unchanged summaries keep the previous rev, so the snapshot file stays identical
  // and clients get a 304.
  const prevSnap = input.previous?.snapshot ?? null;
  const summariesSame =
    prevSnap !== null &&
    prevSnap.ports.length === summaries.length &&
    summaries.every((s) => {
      const p = prevSummaries.get(s.id);
      return p !== undefined && JSON.stringify(p) === JSON.stringify(s);
    });
  const nowSecs = Math.floor(input.now.getTime() / 1000);
  const rev =
    prevSnap && summariesSame ? prevSnap.rev : Math.max(nowSecs, (prevSnap?.rev ?? 0) + 1);

  const files = new Map<string, string>();
  const put = (path: string, value: unknown) => files.set(path, JSON.stringify(value));
  put("v1/snapshot.json", { rev, ports: summaries } satisfies CatalogSnapshot);
  for (const [id, d] of details) {
    put(`v1/ports/${id}.json`, d);
    put(`v1/ports/${id}/releases.json`, input.releases.get(id) ?? []);
  }
  if (input.covers) put("build/steamgriddb.json", input.covers);
  for (const [key, g] of gameEntries) {
    if (!SAFE_NAME.test(key)) {
      warnings.push(`game_key "${key}" can't be a file name; no games/ file for it`);
      continue;
    }
    put(`v1/games/${key}.json`, {
      game_key: key,
      title: gameTitle(g.best.title),
      original_platforms: g.platforms,
      ports: summaries.filter((s) => s.game_key === key).sort(byStars),
    } satisfies Game);
  }

  const hash = createHash("sha256");
  for (const path of [...files.keys()].sort()) {
    hash
      .update(path)
      .update("\0")
      .update(files.get(path) as string)
      .update("\0");
  }
  const content_hash = hash.digest("hex");
  const prevIndex = input.previous?.index ?? null;
  const changed = prevIndex?.content_hash !== content_hash;
  const index: CatalogIndex = {
    schema: 1,
    rev,
    generated_at: changed || !prevIndex ? input.now.toISOString() : prevIndex.generated_at,
    port_count: summaries.length,
    content_hash,
  };
  put("v1/index.json", index);
  return { files, index, changed, warnings, invalidEntries };
}
