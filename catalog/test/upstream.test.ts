import { describe, expect, it } from "vitest";
import {
  UPSTREAM_FILES,
  entryHash,
  gameTitle,
  originalPlatforms,
  parseUpstream,
  portColumns,
  screenshotUrl,
  seedStatus,
} from "../src/upstream.js";
import { syntheticGames, syntheticUpstream } from "./fixtures/index.js";

describe("parseUpstream", () => {
  it("accepts the { games } wrapper and a bare array", () => {
    expect(parseUpstream(syntheticUpstream()).entries).toHaveLength(6);
    expect(parseUpstream(syntheticGames()).entries).toHaveLength(6);
    expect(() => parseUpstream({ nope: 1 })).toThrow(/expected an array/);
  });

  it("drops malformed entries and keeps the most-starred duplicate", () => {
    const games = syntheticGames();
    const dupe = { ...games[0], stars: 1, title: "loser" };
    const winner = { ...games[2], stars: 99_999, title: "winner" };
    const r = parseUpstream([
      ...games,
      dupe,
      winner,
      { id: "", title: "x" },
      null,
      { id: "a b", title: "x", game_key: "k", type: "tool" },
    ]);
    expect(r.invalid).toBe(3);
    expect(r.duplicateIds).toEqual(["castle-quest-port", "star-racer-recomp"]);
    const titles = Object.fromEntries(r.entries.map((e) => [e.id, e.title]));
    expect(titles["star-racer-recomp"]).toBe("Star Racer (Turbo Recomp)");
    expect(titles["castle-quest-port"]).toBe("winner");
  });
});

describe("seedStatus", () => {
  const s = (type: string, release: string | null, playability?: number, completeness?: number) =>
    seedStatus({
      type,
      release,
      ratings: playability ? { playability, ...(completeness ? { completeness } : {}) } : null,
    });

  it("follows the catalog spec table", () => {
    expect(s("source-port", "1", 5, 5)).toBe("perfect");
    expect(s("source-port", "1", 5, 4)).toBe("playable");
    expect(s("source-port", "1", 4)).toBe("in_progress");
    expect(s("source-port", "1", 3)).toBe("in_progress");
    expect(s("source-port", "1", 2)).toBe("boots");
    expect(s("source-port", "1", 1)).toBe("not_playable");
    expect(s("source-port", "1")).toBe("not_playable");
    expect(s("decompilation", null, 5, 5)).toBe("not_playable");
    expect(s("disassembly", null, 4)).toBe("not_playable");
    expect(s("decompilation", "2.0", 5, 4)).toBe("playable");
  });
});

describe("mapping helpers", () => {
  it("splits platforms and strips the port name from titles", () => {
    expect(originalPlatforms("N64 / GameCube")).toEqual(["N64", "GameCube"]);
    expect(originalPlatforms(null)).toEqual([]);
    expect(gameTitle("Star Racer (Turbo Recomp)")).toBe("Star Racer");
    expect(gameTitle("(Only Parens)")).toBe("(Only Parens)");
  });

  it("maps columns defensively", () => {
    const [e] = syntheticGames();
    const cols = portColumns({
      ...e!,
      url: "javascript:alert(1)",
      ratings: { playability: 9, completeness: 5 },
    } as never);
    expect(cols.url).toBeNull();
    expect(cols.ratings).toEqual({ completeness: 5 });
    expect(cols.screenshot_url).toBe(`${UPSTREAM_FILES}screenshots/star-racer-recomp.jpg`);
    expect(cols.play_url).toBeNull();
    expect(cols.last_commit).toBe("2026-10-01T00:00:00.000Z");
  });

  it("links upstream screenshots only as images in the list's repo or over https", () => {
    expect(screenshotUrl("screenshots/a.png")).toBe(`${UPSTREAM_FILES}screenshots/a.png`);
    expect(screenshotUrl("/screenshots/a.webp")).toBe(`${UPSTREAM_FILES}screenshots/a.webp`);
    expect(screenshotUrl("https://example.com/shot.JPG")).toBe("https://example.com/shot.JPG");
    expect(screenshotUrl("http://example.com/shot.jpg")).toBeNull();
    expect(screenshotUrl("../../other/repo/a.png")).toBeNull();
    expect(screenshotUrl("roms/game.z64")).toBeNull();
    expect(screenshotUrl("")).toBeNull();
    expect(screenshotUrl(null)).toBeNull();
  });

  it("hashes independent of key order", () => {
    expect(entryHash({ id: "a", title: "b", game_key: "c", type: "tool" })).toBe(
      entryHash({ type: "tool", game_key: "c", title: "b", id: "a" }),
    );
  });
});
