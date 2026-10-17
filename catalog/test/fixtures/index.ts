/**
 * Synthetic upstream catalog for tests. Invented projects only: no real game names, repos,
 * screenshots or links to game files (docs/legal/policy.md). Shape follows games.json from
 * awesome-decomp-games (the `{ games: [...] }` wrapper).
 */
import { fileURLToPath } from "node:url";

type Entry = Record<string, unknown>;

const base = (id: string, over: Entry): Entry => ({
  id,
  title: `Example ${id}`,
  game_key: id,
  platform: "Example Console",
  type: "source-port",
  url: `https://example.com/${id}`,
  repo: `example-org/${id}`,
  description: null,
  stars: 10,
  forks: 1,
  last_commit: "2026-10-01T00:00:00Z",
  archived: false,
  license: "MIT",
  release: "1.0.0",
  homepage: null,
  play_url: null,
  ratings: { playability: 3, completeness: 3, activity: 3, popularity: 3 },
  score: 3,
  screenshot: null,
  video: null,
  note: null,
  ...over,
});

export function syntheticGames(): Entry[] {
  return [
    base("star-racer-recomp", {
      title: "Star Racer (Turbo Recomp)",
      game_key: "starracer",
      platform: "Example 64 / Example Cube",
      type: "static-recomp",
      stars: 5000,
      description: "Native recompilation of a kart racing game.",
      ratings: { playability: 5, completeness: 5, activity: 5, popularity: 5 },
      score: 4.8,
      video: "https://video.example.com/watch?v=1",
      // A path into the upstream list's repo; no such file exists (synthetic).
      screenshot: "screenshots/star-racer-recomp.jpg",
    }),
    base("star-racer-decomp", {
      title: "Star Racer (decomp)",
      game_key: "starracer",
      platform: "Example 64",
      type: "decompilation",
      stars: 900,
      release: null,
      ratings: { playability: 5, completeness: 4, activity: 4, popularity: 3 },
    }),
    base("castle-quest-port", {
      title: "Castle Quest (Source Port)",
      game_key: "castlequest",
      type: "source-port",
      stars: 1200,
      description: "Open source engine for a castle adventure.",
      ratings: { playability: 5, completeness: 4, activity: 4, popularity: 4 },
    }),
    base("puzzle-blocks-web", {
      title: "Puzzle Blocks (browser)",
      game_key: "puzzleblocks",
      type: "browser-port",
      stars: 300,
      play_url: "https://play.example.com/puzzle",
      ratings: { playability: 4, completeness: 3, activity: 2, popularity: 2 },
    }),
    base("dungeon-remake", {
      title: "Dungeon Crawler (engine remake)",
      game_key: "dungeoncrawler",
      platform: "Example PC",
      type: "engine-remake",
      stars: 450,
      archived: true,
      ratings: { playability: 2, completeness: 2, activity: 1, popularity: 2 },
    }),
    base("space-shooter-disasm", {
      title: "Space Shooter (disassembly)",
      game_key: "spaceshooter",
      type: "disassembly",
      stars: null,
      release: null,
      ratings: { playability: 1, completeness: 1, activity: 1, popularity: 1 },
    }),
  ];
}

export function syntheticUpstream(games = syntheticGames()) {
  return { generated_at: "2026-10-13T00:00:00Z", total: games.length, games };
}

export const manifestsFixtureDir = fileURLToPath(new URL("./manifests/", import.meta.url));
