import { describe, expect, it } from "vitest";
import { buildCatalog } from "../src/build.js";
import { loadManifests } from "../src/manifests.js";
import type { CatalogSnapshot, Game, PortDetail, Release } from "../src/schema/catalog.js";
import type { Manifest } from "../src/schema/manifest.js";
import { syntheticGames, syntheticUpstream } from "./fixtures/index.js";

// Synthetic manifest for a port in the fixture catalog.
const manifest: Manifest = {
  schema: 1,
  id: "castle-quest-port",
  game_key: "castlequest",
  display_name: "Castle Quest",
  status: "perfect",
  status_notes: "Needs your own game files.",
  source: { type: "github_releases", repo: "example-org/castle-quest-port" },
  builds: [
    {
      os: "macos",
      arch: "universal",
      asset: "cq-*-mac.zip",
      archive: "zip",
      launch: { app_bundle: "CQ.app" },
    },
    {
      os: "windows",
      arch: "x86_64",
      asset: "cq-*-win.zip",
      archive: "zip",
      launch: { exe: "cq.exe" },
    },
  ],
};
const loaded = [{ file: "castle-quest-port.yaml", manifest, rev: "abc123", warnings: [] }];

const release = (tag: string, revoked = false): Release => ({
  id: tag,
  port_id: "castle-quest-port",
  tag,
  name: null,
  published_at: "2026-10-01T00:00:00.000Z",
  prerelease: false,
  revoked,
  html_url: `https://example.com/${tag}`,
  assets: [],
});

const t0 = new Date("2026-10-14T00:00:00Z");
const t1 = new Date("2026-10-14T01:00:00Z");
const json = <T>(out: { files: Map<string, string> }, path: string) =>
  JSON.parse(out.files.get(path) as string) as T;

describe("buildCatalog", () => {
  const first = buildCatalog({
    upstream: syntheticUpstream(),
    manifests: loaded,
    releases: new Map([["castle-quest-port", [release("v3", true), release("v2"), release("v1")]]]),
    now: t0,
  });

  it("writes the snapshot, one file per port, releases and games", () => {
    const snap = json<CatalogSnapshot>(first, "v1/snapshot.json");
    expect(snap.rev).toBe(t0.getTime() / 1000);
    expect(snap.ports.map((p) => p.id)).toEqual([...snap.ports.map((p) => p.id)].sort());
    expect(snap.ports).toHaveLength(6);
    expect(first.files.has("v1/ports/star-racer-recomp.json")).toBe(true);
    expect(first.files.has("v1/ports/star-racer-recomp/releases.json")).toBe(true);
    const game = json<Game>(first, "v1/games/starracer.json");
    expect(game.title).toBe("Star Racer");
    expect(game.original_platforms).toEqual(["Example 64", "Example Cube"]);
    expect(game.ports.map((p) => p.id)).toEqual(["star-racer-recomp", "star-racer-decomp"]);
    expect(first.changed).toBe(true);
    expect(first.index.port_count).toBe(6);
  });

  it("applies the manifest over the upstream entry", () => {
    const d = json<PortDetail>(first, "v1/ports/castle-quest-port.json");
    expect(d.title).toBe("Castle Quest");
    expect(d.status).toBe("perfect");
    expect(d.os_status).toEqual([
      { os: "windows", status: "perfect", source: "manifest" },
      { os: "macos", status: "perfect", source: "manifest" },
    ]);
    expect(d.manifest_rev).toBe("abc123");
    expect(d.status_notes).toBe("Needs your own game files.");
    expect(d.latest_releases.map((r) => r.tag)).toEqual(["v2", "v1"]);
    expect(json<Release[]>(first, "v1/ports/castle-quest-port/releases.json")).toHaveLength(3);
    // No upstream screenshot and no preview images given.
    expect(d.screenshot_url).toBeNull();
  });

  it("uses the upstream screenshot, else the repo's preview image, else keeps the published one", () => {
    const shot = (out: typeof first, id: string) =>
      json<PortDetail>(out, `v1/ports/${id}.json`).screenshot_url;
    const preview = "https://repository-images.githubusercontent.com/1/abc";
    const withPreviews = buildCatalog({
      upstream: syntheticUpstream(),
      manifests: loaded,
      releases: new Map(),
      previews: new Map([
        ["example-org/castle-quest-port", preview],
        ["example-org/star-racer-recomp", "https://repository-images.githubusercontent.com/2/x"],
      ]),
      now: t0,
    });
    // Upstream screenshot wins over the preview image.
    expect(shot(withPreviews, "star-racer-recomp")).toMatch(/screenshots\/star-racer-recomp\.jpg$/);
    expect(shot(withPreviews, "castle-quest-port")).toBe(preview);
    expect(shot(withPreviews, "star-racer-decomp")).toBeNull();

    // Previews unavailable (no token, GitHub error): keep what was published.
    const kept = buildCatalog({
      upstream: syntheticUpstream(),
      manifests: loaded,
      releases: new Map(),
      previews: null,
      previous: {
        index: withPreviews.index,
        snapshot: json<CatalogSnapshot>(withPreviews, "v1/snapshot.json"),
      },
      now: t1,
    });
    expect(shot(kept, "castle-quest-port")).toBe(preview);
  });

  it("falls back to the game's SteamGridDB cover and publishes the lookup cache", () => {
    const cover = "https://cdn2.steamgriddb.com/grid/1.png";
    const decomp = json<PortDetail>(first, "v1/ports/star-racer-decomp.json");
    const covers = {
      schema: 1 as const,
      games: { [decomp.game_key]: { id: 1, url: cover, checked: "2026-10-14" } },
    };
    const out = buildCatalog({
      upstream: syntheticUpstream(),
      manifests: loaded,
      releases: new Map(),
      previews: new Map(),
      covers,
      now: t0,
    });
    expect(json<PortDetail>(out, "v1/ports/star-racer-decomp.json").screenshot_url).toBe(cover);
    // The list's screenshot still wins for the other port of the same game.
    expect(json<PortDetail>(out, "v1/ports/star-racer-recomp.json").screenshot_url).toMatch(
      /star-racer-recomp\.jpg$/,
    );
    expect(json(out, "build/steamgriddb.json")).toEqual(covers);
    expect(first.files.has("build/steamgriddb.json")).toBe(false);
  });

  it("lets a covers.yaml entry replace the list screenshot", () => {
    const cover = "https://cdn2.steamgriddb.com/grid/2.png";
    const recomp = json<PortDetail>(first, "v1/ports/star-racer-recomp.json");
    expect(recomp.screenshot_url).toMatch(/star-racer-recomp\.jpg$/);
    const build = (url: string | null) =>
      buildCatalog({
        upstream: syntheticUpstream(),
        manifests: loaded,
        releases: new Map(),
        previews: new Map(),
        covers: {
          schema: 1,
          games: { [recomp.game_key]: { id: 2, url, checked: "2026-10-15" } },
        },
        coverOverrides: new Set([recomp.game_key]),
        now: t0,
      });
    const shotOf = (out: ReturnType<typeof build>) =>
      json<PortDetail>(out, "v1/ports/star-racer-recomp.json").screenshot_url;
    expect(shotOf(build(cover))).toBe(cover);
    // `none` (or no cover found yet) leaves the generated cover, not the list screenshot.
    expect(shotOf(build(null))).toBeNull();
  });

  it("keeps rev, updated_at and generated_at when nothing changed", () => {
    const again = buildCatalog({
      upstream: syntheticUpstream(),
      manifests: loaded,
      releases: new Map([
        ["castle-quest-port", [release("v3", true), release("v2"), release("v1")]],
      ]),
      previous: {
        index: first.index,
        snapshot: json<CatalogSnapshot>(first, "v1/snapshot.json"),
      },
      now: t1,
    });
    expect(again.changed).toBe(false);
    expect(again.files.get("v1/snapshot.json")).toBe(first.files.get("v1/snapshot.json"));
    expect(again.index).toEqual(first.index);
  });

  it("a new release changes the content but not the snapshot", () => {
    const next = buildCatalog({
      upstream: syntheticUpstream(),
      manifests: loaded,
      releases: new Map([["castle-quest-port", [release("v4"), release("v2")]]]),
      previous: { index: first.index, snapshot: json<CatalogSnapshot>(first, "v1/snapshot.json") },
      now: t1,
    });
    expect(next.changed).toBe(true);
    expect(next.files.get("v1/snapshot.json")).toBe(first.files.get("v1/snapshot.json"));
    expect(next.index.generated_at).toBe(t1.toISOString());
  });

  it("a changed port bumps rev and only its updated_at", () => {
    const games = syntheticGames().map((g) =>
      g.id === "dungeon-remake" ? { ...g, stars: 999 } : g,
    );
    const next = buildCatalog({
      upstream: syntheticUpstream(games),
      manifests: loaded,
      releases: new Map(),
      previous: { index: first.index, snapshot: json<CatalogSnapshot>(first, "v1/snapshot.json") },
      now: t1,
    });
    const snap = json<CatalogSnapshot>(next, "v1/snapshot.json");
    expect(snap.rev).toBe(t1.getTime() / 1000);
    const updated = Object.fromEntries(snap.ports.map((p) => [p.id, p.updated_at]));
    expect(updated["dungeon-remake"]).toBe(t1.toISOString());
    expect(updated["star-racer-recomp"]).toBe(t0.toISOString());
  });

  it("warns about manifests for unknown ports", async () => {
    const { valid } = await loadManifests(new URL("../../manifests", import.meta.url).pathname);
    const out = buildCatalog({
      upstream: syntheticUpstream(),
      manifests: valid,
      releases: new Map(),
      now: t0,
    });
    expect(out.warnings.some((w) => w.includes("no port with this id"))).toBe(true);
  });
});
