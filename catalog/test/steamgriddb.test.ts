import { describe, expect, it } from "vitest";
import {
  type CoverCache,
  lookupCover,
  normalizeTitle,
  parseOverrides,
  pickGame,
  pickGrid,
  readCache,
  titleVariants,
  updateCovers,
} from "../src/steamgriddb.js";

const cdn = (n: number) => `https://cdn2.steamgriddb.com/grid/${n}.png`;

/** A fake SteamGridDB: search results by term, grids by game id. */
function fakeApi(
  games: Record<string, { id: number; name: string }[]>,
  grids: Record<number, { id: number; score?: number; url: string }[]>,
  calls: string[] = [],
): typeof fetch {
  return async (input, init) => {
    const url = new URL(String(input));
    calls.push(url.pathname + url.search);
    expect(new Headers(init?.headers).get("authorization")).toBe("Bearer k");
    const search = /\/search\/autocomplete\/(.+)$/.exec(url.pathname);
    if (search) {
      const term = decodeURIComponent(search[1] as string);
      return Response.json({ success: true, data: games[term] ?? [] });
    }
    const grid = /\/grids\/game\/(\d+)$/.exec(url.pathname);
    const list = grid ? grids[Number(grid[1])] : undefined;
    if (!list) return Response.json({ success: false }, { status: 404 });
    return Response.json({ success: true, data: list });
  };
}

describe("steamgriddb", () => {
  it("matches titles ignoring case and punctuation, never the nearest guess", () => {
    expect(normalizeTitle("Example: Quest & Co.")).toBe("examplequestandco");
    const results = [
      { id: 1, name: "Example Quest II" },
      { id: 2, name: "example quest" },
    ];
    expect(pickGame(results, "Example Quest")).toBe(2);
    expect(pickGame(results, "Example Quest III")).toBeNull();
    expect(pickGame(results, "!!!")).toBeNull();
  });

  it("tries the title without its trailing notes, the whole title first", () => {
    expect(titleVariants("Example Racer (Racer GBA (racergba))")).toEqual([
      "Example Racer (Racer GBA (racergba))",
      "Example Racer",
    ]);
    expect(titleVariants("Example Kart (2012) (kart-pc-port)")).toEqual([
      "Example Kart (2012) (kart-pc-port)",
      "Example Kart (2012)",
      "Example Kart",
    ]);
    expect(titleVariants("(only a note)")).toEqual(["(only a note)"]);
    expect(titleVariants("Example (Deluxe) Kart")).toEqual(["Example (Deluxe) Kart"]);
    const results = [
      { id: 1, name: "Example Kart" },
      { id: 2, name: "Example Kart (2012)" },
    ];
    expect(pickGame(results, "Example Kart (2012) (kart-pc-port)")).toBe(2);
    expect(pickGame([results[0]!], "Example Kart (2012) (kart-pc-port)")).toBe(1);
  });

  it("searches again without the notes when the whole title finds nothing", async () => {
    const calls: string[] = [];
    const api = fakeApi(
      { "Example Racer": [{ id: 7, name: "Example Racer" }] },
      { 7: [{ id: 1, url: cdn(1) }] },
      calls,
    );
    expect(await lookupCover("Example Racer (racergba)", { key: "k", fetchFn: api })).toEqual({
      id: 7,
      url: cdn(1),
    });
    expect(calls).toHaveLength(3);
  });

  it("picks the best grid on the CDN and keeps the current one while it's listed", () => {
    const grids = [
      { id: 3, score: 1, url: cdn(3) },
      { id: 2, score: 5, url: cdn(2) },
      { id: 1, score: 9, url: "https://evil.example/x.png" },
    ];
    expect(pickGrid(grids)).toBe(cdn(2));
    expect(pickGrid(grids, cdn(3))).toBe(cdn(3));
    expect(pickGrid(grids, cdn(9))).toBe(cdn(2));
    expect(pickGrid([])).toBeNull();
  });

  it("looks a game up by title, or by id when it is known", async () => {
    const calls: string[] = [];
    const api = fakeApi(
      { "Example Quest": [{ id: 7, name: "Example Quest" }] },
      { 7: [{ id: 1, url: cdn(1) }] },
      calls,
    );
    expect(await lookupCover("Example Quest", { key: "k", fetchFn: api })).toEqual({
      id: 7,
      url: cdn(1),
    });
    expect(calls[1]).toContain("nsfw=false");
    expect(await lookupCover("Nothing", { key: "k", fetchFn: api })).toEqual({
      id: null,
      url: null,
    });
    calls.length = 0;
    expect(await lookupCover("ignored", { key: "k", fetchFn: api }, { id: 99 })).toEqual({
      id: 99,
      url: null,
    });
    expect(calls).toEqual([expect.stringMatching(/^\/api\/v2\/grids\/game\/99\?/)]);
  });

  it("reads overrides and refuses anything else", () => {
    const o = parseOverrides("example-quest: 42\nother-game: none\n");
    expect([...o]).toEqual([
      ["example-quest", 42],
      ["other-game", "none"],
    ]);
    expect(parseOverrides("")).toEqual(new Map());
    expect(() => parseOverrides("x: maybe")).toThrow(/x:/);
    expect(() => parseOverrides("- 1")).toThrow();
  });

  it("drops cache entries it can't trust", () => {
    const cache = readCache({
      schema: 1,
      games: {
        good: { id: 1, url: cdn(1), checked: "2026-10-01" },
        miss: { id: null, url: null, checked: "2026-10-01" },
        offsite: { id: 1, url: "https://evil.example/x.png", checked: "2026-10-01" },
        __proto__x: { id: 1, url: cdn(1), checked: "2026-10-01" },
        undated: { id: 1, url: cdn(1) },
      },
    });
    expect(Object.keys(cache.games)).toEqual(["good", "miss"]);
    expect(readCache(null).games).toEqual({});
    expect(readCache({ schema: 2, games: {} }).games).toEqual({});
  });

  it("looks up new and stale games within the budget and honors overrides", async () => {
    const calls: string[] = [];
    const api = fakeApi(
      {
        "Alpha Game": [{ id: 1, name: "Alpha Game" }],
        "Beta Game": [{ id: 2, name: "Beta Game" }],
        "Gamma Game": [{ id: 3, name: "Gamma Game" }],
      },
      {
        1: [{ id: 10, url: cdn(10) }],
        2: [{ id: 20, url: cdn(20) }],
        5: [{ id: 50, url: cdn(50) }],
      },
      calls,
    );
    const cache: CoverCache = {
      schema: 1,
      games: {
        fresh: { id: 9, url: cdn(90), checked: "2026-10-10" },
        stale: { id: 4, url: cdn(40), checked: "2026-09-01" },
        gone: { id: 8, url: cdn(80), checked: "2026-10-10" },
        hidden: { id: 6, url: cdn(60), checked: "2026-10-10" },
      },
    };
    const wanted = new Map([
      ["alpha", "Alpha Game"],
      ["beta", "Beta Game"],
      ["gamma", "Gamma Game"],
      ["fresh", "Fresh"],
      ["stale", "Stale"],
      ["hidden", "Hidden"],
      ["forced", "Forced"],
    ]);
    const out = await updateCovers({
      wanted,
      cache,
      overrides: new Map<string, number | "none">([
        ["hidden", "none"],
        ["forced", 5],
      ]),
      opts: { key: "k", fetchFn: api },
      today: "2026-10-14",
      budget: 4,
    });
    // Never-checked games first (alpha, beta, forced, gamma by key), then stale ones: the
    // budget of 4 leaves "stale" for a later build.
    expect(out.games).toEqual({
      alpha: { id: 1, url: cdn(10), checked: "2026-10-14" },
      beta: { id: 2, url: cdn(20), checked: "2026-10-14" },
      forced: { id: 5, url: cdn(50), checked: "2026-10-14" },
      // Matched, but no grids yet: checked again sooner, by id.
      gamma: { id: 3, url: null, checked: "2026-10-14" },
      fresh: { id: 9, url: cdn(90), checked: "2026-10-10" },
      stale: { id: 4, url: cdn(40), checked: "2026-09-01" },
      hidden: { id: null, url: null, checked: "2026-10-10" },
    });
    expect(calls.some((c) => c.includes("Forced"))).toBe(false);
  });

  it("looks up covers.yaml ids before the backlog of never-checked games", async () => {
    const api = fakeApi(
      { "Aaa Game": [{ id: 1, name: "Aaa Game" }] },
      { 7: [{ id: 70, url: cdn(70) }] },
    );
    const out = await updateCovers({
      wanted: new Map([
        ["aaa", "Aaa Game"],
        ["zzz", "Zzz Game"],
      ]),
      cache: { schema: 1, games: { zzz: { id: null, url: null, checked: "2026-10-14" } } },
      overrides: new Map([["zzz", 7]]),
      opts: { key: "k", fetchFn: api },
      today: "2026-10-15",
      budget: 1,
    });
    expect(out.games.zzz).toEqual({ id: 7, url: cdn(70), checked: "2026-10-15" });
    expect(out.games.aaa).toBeUndefined();
  });

  it("stops at the first failed request and keeps the cache", async () => {
    const failing: typeof fetch = async () => new Response("", { status: 401 });
    const logs: string[] = [];
    const cache: CoverCache = {
      schema: 1,
      games: { kept: { id: 1, url: cdn(1), checked: "2026-01-01" } },
    };
    const out = await updateCovers({
      wanted: new Map([
        ["kept", "Kept"],
        ["new", "New"],
      ]),
      cache,
      overrides: new Map(),
      opts: { key: "k", fetchFn: failing },
      today: "2026-10-14",
      log: (m) => logs.push(m),
    });
    expect(out.games).toEqual(cache.games);
    expect(logs).toEqual([expect.stringMatching(/401/)]);
  });
});
