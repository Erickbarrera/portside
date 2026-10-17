// Synthetic catalog rows for local development. Not real projects: no game names,
// art, repos or asset links (see docs/legal/policy.md). Shapes follow PortSummary in
// src/lib/catalog.ts.
const os = (windows, macos, linux, android) =>
  Object.entries({ windows, macos, linux, android })
    .filter(([, status]) => status)
    .map(([name, status]) => ({ os: name, status, source: "manifest" }));

export const ports = [
  {
    id: "example-source-port",
    game_key: "example-platformer",
    title: "Example Platformer (source port)",
    type: "source-port",
    status: "perfect",
    os_status: os("perfect", "perfect", "perfect", "playable"),
    stars: 4821,
    archived: false,
    screenshot_url: null,
    play_url: null,
    updated_at: "2026-10-09T06:00:00Z",
  },
  {
    id: "example-static-recomp",
    game_key: "example-racer",
    title: "Example Racer (static recomp)",
    type: "static-recomp",
    status: "playable",
    os_status: os("playable", "in_progress", "playable", null),
    stars: 2310,
    archived: false,
    screenshot_url: null,
    play_url: null,
    updated_at: "2026-10-08T12:30:00Z",
  },
  {
    id: "example-engine-remake",
    game_key: "example-rpg",
    title: "Example RPG (engine remake)",
    type: "engine-remake",
    status: "in_progress",
    os_status: os("in_progress", "boots", "in_progress", null),
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
    updated_at: "2026-09-02T07:45:00Z",
  },
  {
    id: "example-archived-port",
    game_key: "example-shooter",
    title: "Example Shooter (source port)",
    type: "source-port",
    status: "boots",
    os_status: os("boots", null, "boots", null),
    stars: 142,
    archived: true,
    screenshot_url: null,
    play_url: null,
    updated_at: "2025-03-11T10:00:00Z",
  },
];

// Filler rows so paging ("Load more") can be tried against the mock: 150 numbered,
// obviously synthetic entries after the hand-written ones above.
const types = ["source-port", "static-recomp", "decompilation", "engine-remake"];
const statuses = ["perfect", "playable", "in_progress", "boots", "not_playable"];
for (let i = 1; i <= 150; i++) {
  const n = String(i).padStart(3, "0");
  const status = statuses[i % statuses.length];
  ports.push({
    id: `sample-port-${n}`,
    game_key: `sample-game-${n}`,
    title: `Sample Game ${n}`,
    type: types[i % types.length],
    status,
    os_status: os(status, i % 3 ? status : null, status, null),
    stars: 100 - (i % 100),
    archived: false,
    screenshot_url: null,
    play_url: null,
    updated_at: "2026-09-01T00:00:00Z",
  });
}
