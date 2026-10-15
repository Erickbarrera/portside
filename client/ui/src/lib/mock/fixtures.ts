// Synthetic catalog rows for UI development and tests. Not real projects: no game names,
// art, repos or asset links (docs/legal/policy.md). Mirrors web/scripts/fixtures.mjs.
import type { Os, OsStatus, PlayableStatus, PortDetail_Serialize, PortSummary } from "../ipc";

const os = (s: Partial<Record<Os, PlayableStatus>>): OsStatus[] =>
  (Object.entries(s) as [Os, PlayableStatus][]).map(([name, status]) => ({
    os: name,
    status,
    source: "manifest",
  }));

/** A made-up "screenshot": abstract shapes, clearly labelled, no game art. */
function shot(hue: number, label: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 360">
<rect width="640" height="360" fill="hsl(${hue} 45% 22%)"/>
<rect y="250" width="640" height="110" fill="hsl(${hue + 30} 40% 30%)"/>
<circle cx="500" cy="90" r="46" fill="hsl(${hue + 60} 80% 70%)"/>
<rect x="90" y="190" width="70" height="60" rx="8" fill="hsl(${hue + 180} 60% 60%)"/>
<rect x="260" y="150" width="160" height="16" rx="8" fill="hsl(${hue} 30% 55%)"/>
<text x="24" y="40" font-family="sans-serif" font-size="20" fill="white" opacity="0.7">${label}</text>
</svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

export const ports: PortSummary[] = [
  {
    id: "example-source-port",
    game_key: "example-platformer",
    title: "Example Platformer (source port)",
    type: "source-port",
    status: "perfect",
    os_status: os({ windows: "perfect", macos: "perfect", linux: "perfect", android: "playable" }),
    stars: 4821,
    archived: false,
    screenshot_url: shot(200, "Synthetic screenshot"),
    play_url: null,
    updated_at: "2026-10-09T06:00:00Z",
  },
  {
    id: "example-static-recomp",
    game_key: "example-racer",
    title: "Example Racer (static recomp)",
    type: "static-recomp",
    status: "playable",
    os_status: os({ windows: "playable", macos: "in_progress", linux: "playable" }),
    stars: 2310,
    archived: false,
    screenshot_url: shot(20, "Synthetic screenshot"),
    play_url: null,
    updated_at: "2026-10-08T12:30:00Z",
  },
  {
    id: "example-engine-remake",
    game_key: "example-rpg",
    title: "Example RPG (engine remake)",
    type: "engine-remake",
    status: "in_progress",
    os_status: os({ windows: "in_progress", macos: "boots", linux: "in_progress" }),
    stars: 905,
    archived: false,
    screenshot_url: null,
    play_url: null,
    updated_at: "2026-10-01T09:15:00Z",
  },
  {
    id: "example-browser-port",
    game_key: "example-puzzle",
    title: "Example Puzzle (browser port)",
    type: "browser-port",
    status: "playable",
    os_status: [],
    stars: 377,
    archived: false,
    screenshot_url: null,
    play_url: "https://example.com/play",
    updated_at: "2026-09-20T18:00:00Z",
  },
  {
    id: "example-decomp",
    game_key: "example-adventure",
    title: "Example Adventure (decompilation)",
    type: "decompilation",
    status: "not_playable",
    os_status: [],
    stars: 1288,
    archived: false,
    screenshot_url: null,
    play_url: null,
    updated_at: "2026-09-12T10:00:00Z",
  },
  {
    id: "example-archived",
    game_key: "example-shooter",
    title: "Example Shooter (disassembly)",
    type: "disassembly",
    status: "boots",
    os_status: [],
    stars: 64,
    archived: true,
    screenshot_url: null,
    play_url: null,
    updated_at: "2026-06-02T08:00:00Z",
  },
];

export function portDetail(id: string): PortDetail_Serialize | null {
  const summary = ports.find((p) => p.id === id);
  if (!summary) return null;
  const hasBuilds = summary.os_status.length > 0;
  return {
    ...summary,
    description: "A synthetic port used to develop the Portside client. It does not exist.",
    repo: `https://github.com/example/${summary.id}`,
    homepage: null,
    license: "MIT",
    last_commit: summary.updated_at,
    upstream_score: 72.5,
    ratings: { playability: 4, completeness: 4, activity: 3, popularity: 3 },
    video_url: null,
    note: null,
    status_notes: summary.status === "in_progress" ? "Some menus are missing." : null,
    manifest: hasBuilds
      ? {
          schema: 1,
          id: summary.id,
          game_key: summary.game_key,
          status: summary.status,
          source: { type: "github_releases", repo: `example/${summary.id}` },
          builds: summary.os_status.map((s) => ({
            os: s.os,
            arch: s.os === "macos" ? "universal" : "x86_64",
            asset: `${summary.id}-${s.os}-*.zip`,
            archive: "zip",
          })),
        }
      : null,
    manifest_rev: hasBuilds ? "synthetic" : null,
    verified: false,
    latest_releases: hasBuilds
      ? [
          {
            id: `${summary.id}-v1.2.0`,
            port_id: summary.id,
            tag: "v1.2.0",
            name: "Version 1.2.0",
            published_at: "2026-10-01T00:00:00Z",
            prerelease: false,
            revoked: false,
            html_url: `https://github.com/example/${summary.id}/releases/tag/v1.2.0`,
            assets: summary.os_status.map((s) => ({
              os: s.os,
              arch: s.os === "macos" ? "universal" : "x86_64",
              name: `${summary.id}-${s.os}-v1.2.0.zip`,
              url: `https://github.com/example/${summary.id}/releases/download/v1.2.0/${summary.id}-${s.os}-v1.2.0.zip`,
              size: 48_000_000,
              sha256: null,
            })),
          },
        ]
      : [],
  };
}
