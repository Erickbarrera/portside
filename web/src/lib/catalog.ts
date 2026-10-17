// The static Portside catalog (../docs/specs/static-catalog.md), published to GitHub Pages
// of this repository. The site reads it at build time and again in the browser,
// so it never depends on a Portside server and always shows the live catalog.

export type Os = "windows" | "macos" | "linux" | "android";
export type PlayableStatus = "perfect" | "playable" | "in_progress" | "boots" | "not_playable";
export type PortType =
  | "static-recomp"
  | "source-port"
  | "engine-remake"
  | "browser-port"
  | "decompilation"
  | "disassembly"
  | "tool";

export interface OsStatus {
  os: Os;
  status: PlayableStatus;
  source: "curator" | "manifest" | "seed";
}

export interface PortSummary {
  id: string;
  game_key: string;
  title: string;
  type: PortType;
  status: PlayableStatus;
  os_status: OsStatus[];
  stars: number | null;
  archived: boolean;
  screenshot_url: string | null;
  play_url: string | null;
  updated_at: string;
}

export interface CatalogSnapshot {
  rev: number;
  ports: PortSummary[];
}

/** Published catalog. [TBD: a custom domain once one is bought] */
export const DEFAULT_CATALOG_URL = "https://erickbarrera.github.io/portside";

/**
 * Base URL of the catalog. The browser uses PUBLIC_PORTSIDE_CATALOG_URL (may be a path on this
 * site, like `pnpm dev:mock` uses); the build uses PORTSIDE_CATALOG_URL, falling back to the
 * same. Both default to the published catalog.
 */
const browserUrl = import.meta.env.PUBLIC_PORTSIDE_CATALOG_URL || DEFAULT_CATALOG_URL;
export const catalogUrl = (
  (import.meta.env.SSR && import.meta.env.PORTSIDE_CATALOG_URL) || browserUrl
).replace(/\/+$/, "");

export async function fetchSnapshot(signal?: AbortSignal): Promise<CatalogSnapshot> {
  const res = await fetch(`${catalogUrl}/v1/snapshot.json`, { signal: signal ?? null });
  if (!res.ok) throw new Error(`HTTP ${res.status} from ${catalogUrl}/v1/snapshot.json`);
  return (await res.json()) as CatalogSnapshot;
}

const fold = (s: string) =>
  s
    .normalize("NFKD")
    .replace(/\p{M}/gu, "")
    .toLowerCase();

const byStars = (a: PortSummary, b: PortSummary) =>
  (b.stars ?? -1) - (a.stars ?? -1) || (a.id < b.id ? -1 : 1);

/**
 * Ports matching `query`, best first. Every word must appear in the title or id; a title that
 * starts with the query beats one with a word starting with it, which beats a plain substring.
 * Ties go to the most-starred port. Without a query: all ports by stars.
 */
export function searchPorts(ports: PortSummary[], query: string): PortSummary[] {
  const q = fold(query.trim());
  if (!q) return [...ports].sort(byStars);
  const words = q.split(/\s+/);
  const scored: { p: PortSummary; score: number }[] = [];
  for (const p of ports) {
    const title = fold(p.title);
    const hay = `${title} ${p.id} ${p.game_key}`;
    if (!words.every((w) => hay.includes(w))) continue;
    const score = title.startsWith(q)
      ? 3
      : new RegExp(`(^|[^\\p{L}\\p{N}])${q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`, "u").test(title)
        ? 2
        : title.includes(q)
          ? 1
          : 0;
    scored.push({ p, score });
  }
  return scored.sort((a, b) => b.score - a.score || byStars(a.p, b.p)).map((s) => s.p);
}
