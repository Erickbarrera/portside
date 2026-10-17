/**
 * awesome-decomp-games (games.json, CC0) → Portside catalog rows.
 * Spec: docs/specs/catalog.md ("Upstream source", "Playable status").
 */
import { createHash } from "node:crypto";
import { PORT_TYPE_VALUES, type PlayableStatus, type PortType } from "./schema/enums.js";

export interface UpstreamEntry {
  id: string;
  title: string;
  game_key: string;
  platform?: string | null;
  type: string;
  url?: string | null;
  repo?: string | null;
  description?: string | null;
  stars?: number | null;
  last_commit?: string | null;
  archived?: boolean | null;
  license?: string | null;
  release?: string | null;
  homepage?: string | null;
  play_url?: string | null;
  ratings?: Partial<
    Record<"playability" | "completeness" | "activity" | "popularity", number>
  > | null;
  score?: number | null;
  screenshot?: string | null;
  video?: string | null;
  note?: string | null;
  [key: string]: unknown;
}

export interface ParsedUpstream {
  entries: UpstreamEntry[];
  /** Entries dropped because a required field was missing or malformed. */
  invalid: number;
  /** Ids that appeared more than once; the entry with the most stars was kept. */
  duplicateIds: string[];
}

const isStr = (v: unknown): v is string => typeof v === "string" && v.length > 0;
const ID_RE = /^[a-z0-9][a-z0-9._-]*$/i;

/**
 * Accepts the current `{ games: [...] }` wrapper and the older bare array. Entries without a
 * usable id, title, game_key or type are dropped; duplicate ids keep the most-starred entry.
 */
export function parseUpstream(json: unknown): ParsedUpstream {
  const list: unknown = Array.isArray(json)
    ? json
    : json && typeof json === "object"
      ? (json as { games?: unknown }).games
      : null;
  if (!Array.isArray(list)) throw new Error("games.json: expected an array or { games: [...] }");
  const byId = new Map<string, UpstreamEntry>();
  const dupes = new Set<string>();
  let invalid = 0;
  for (const raw of list) {
    if (!raw || typeof raw !== "object") {
      invalid++;
      continue;
    }
    const e = raw as UpstreamEntry;
    if (
      !isStr(e.id) ||
      !ID_RE.test(e.id) ||
      !isStr(e.title) ||
      !isStr(e.game_key) ||
      !isStr(e.type)
    ) {
      invalid++;
      continue;
    }
    const prev = byId.get(e.id);
    if (prev) {
      dupes.add(e.id);
      if ((e.stars ?? -1) <= (prev.stars ?? -1)) continue;
    }
    byId.set(e.id, e);
  }
  return { entries: [...byId.values()], invalid, duplicateIds: [...dupes].sort() };
}

/** Stable content hash of one entry, for change detection between runs. */
export function entryHash(entry: UpstreamEntry): string {
  return createHash("sha256").update(canonicalJson(entry)).digest("hex");
}

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value && typeof value === "object") {
    const keys = Object.keys(value).sort();
    return `{${keys.map((k) => `${JSON.stringify(k)}:${canonicalJson((value as Record<string, unknown>)[k])}`).join(",")}}`;
  }
  return JSON.stringify(value ?? null);
}

/** Seed status for ports with no manifest (docs/specs/catalog.md, "Playable status"). */
export function seedStatus(
  entry: Pick<UpstreamEntry, "type" | "release" | "ratings">,
): PlayableStatus {
  if ((entry.type === "decompilation" || entry.type === "disassembly") && !entry.release) {
    return "not_playable";
  }
  const p = entry.ratings?.playability;
  const c = entry.ratings?.completeness;
  if (p === 5) return c === 5 ? "perfect" : "playable";
  if (p === 3 || p === 4) return "in_progress";
  if (p === 2) return "boots";
  // playability 1 or unrated: nothing known to run.
  return "not_playable";
}

export function portType(type: string): PortType | null {
  return (PORT_TYPE_VALUES as readonly string[]).includes(type) ? (type as PortType) : null;
}

/** "N64 / GameCube" → ["N64", "GameCube"]. */
export function originalPlatforms(platform: string | null | undefined): string[] {
  if (!platform) return [];
  return platform
    .split("/")
    .map((s) => s.trim())
    .filter(Boolean);
}

/** "Game Title (Port Name)" → "Game Title". */
export function gameTitle(portTitle: string): string {
  const stripped = portTitle.replace(/\s*\([^()]*\)\s*$/, "").trim();
  return stripped || portTitle;
}

const clampRating = (v: unknown) =>
  typeof v === "number" && Number.isInteger(v) && v >= 1 && v <= 5 ? v : undefined;

const httpUrl = (v: unknown): string | null => {
  if (!isStr(v)) return null;
  try {
    const u = new URL(v);
    return u.protocol === "https:" || u.protocol === "http:" ? v : null;
  } catch {
    return null;
  }
};

const isoDate = (v: unknown): string | null =>
  isStr(v) && !Number.isNaN(Date.parse(v)) ? new Date(v).toISOString() : null;

/** Where the upstream list keeps its files; `screenshot` paths are relative to it. */
export const UPSTREAM_FILES =
  "https://raw.githubusercontent.com/SubmitGame/awesome-decomp-games/main/";
const IMAGE = /\.(jpe?g|png|webp|gif)$/i;

/**
 * The upstream list's screenshot as a link (docs/legal/policy.md rule 6: linked, never
 * copied). Relative paths resolve against the list's own repo; only image files pass.
 */
export function screenshotUrl(v: unknown): string | null {
  if (!isStr(v) || !v.trim()) return null;
  const raw = v.trim();
  let url: URL;
  try {
    url = /^https?:\/\//i.test(raw)
      ? new URL(raw)
      : new URL(raw.replace(/^\/+/, ""), UPSTREAM_FILES);
  } catch {
    return null;
  }
  if (url.protocol !== "https:") return null;
  if (!url.href.startsWith(UPSTREAM_FILES) && !/^https?:\/\//i.test(raw)) return null;
  return IMAGE.test(url.pathname) ? url.href : null;
}

/** Upstream entry → `ports` columns (everything except status, which depends on manifests). */
export function portColumns(e: UpstreamEntry) {
  const ratings: Record<string, number> = {};
  for (const k of ["playability", "completeness", "activity", "popularity"] as const) {
    const r = clampRating(e.ratings?.[k]);
    if (r !== undefined) ratings[k] = r;
  }
  return {
    id: e.id,
    game_key: e.game_key,
    title: e.title,
    type: e.type,
    description: isStr(e.description) ? e.description : null,
    repo: isStr(e.repo) ? e.repo : null,
    url: httpUrl(e.url),
    homepage: httpUrl(e.homepage),
    license: isStr(e.license) ? e.license : null,
    archived: e.archived === true,
    stars: typeof e.stars === "number" && Number.isFinite(e.stars) ? Math.trunc(e.stars) : null,
    last_commit: isoDate(e.last_commit),
    upstream_score: typeof e.score === "number" && Number.isFinite(e.score) ? e.score : null,
    ratings: Object.keys(ratings).length ? ratings : null,
    screenshot_url: screenshotUrl(e.screenshot),
    video_url: httpUrl(e.video),
    note: isStr(e.note) ? e.note : null,
    play_url: e.type === "browser-port" ? httpUrl(e.play_url) : null,
  };
}
