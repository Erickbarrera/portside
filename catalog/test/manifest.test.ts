import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { ManifestSchema, validateManifest } from "../src/index.js";

// Synthetic manifest: invented project, no real game names, repos or hashes.
const valid = () => ({
  schema: 1,
  id: "example-source-port",
  game_key: "example-platformer",
  status: "playable",
  source: { type: "github_releases", repo: "example-org/example-port", channel: "stable" },
  builds: [
    {
      os: "windows",
      arch: "x86_64",
      asset: "Example-*-Win64.zip",
      archive: "zip",
      strip_root: true,
      launch: { exe: "example.exe", args: [] },
      checksum: "upstream",
    },
    {
      os: "linux",
      arch: "x86_64",
      asset: "Example-*-Linux.zip",
      archive: "zip",
      launch: { exe: "example.appimage", chmod_x: true },
    },
    {
      os: "macos",
      arch: "universal",
      asset: "Example-*-Mac.zip",
      archive: "zip",
      launch: { app_bundle: "Example.app" },
      unsigned: true,
    },
    {
      os: "android",
      arch: "arm64",
      asset: "Example-*.apk",
      archive: "apk",
      android_package: "org.example.port",
    },
  ],
  assets_required: [
    {
      id: "game-data",
      label: "Game data file",
      accept: [{ sha1: "0".repeat(40), name: "Synthetic dump" }],
      import: { mode: "copy", dest: "{install}/" },
    },
  ],
  saves: {
    root: { windows: "{install}/Save/", linux: "{xdg_data}/example/Save/" },
    include: ["**/*.sav"],
  },
  multiplayer: "none",
  links: { homepage: "https://example.com", discord: null },
});

describe("validateManifest", () => {
  it("accepts a well-formed manifest", () => {
    const r = validateManifest(valid());
    expect(r).toMatchObject({ ok: true, warnings: [] });
  });

  it("reports schema errors with JSON pointer paths", () => {
    const m = valid() as Record<string, unknown>;
    m.status = "great";
    (m.builds as Array<Record<string, unknown>>)[0]!.os = "dos";
    const r = validateManifest(m);
    expect(r.ok).toBe(false);
    if (!r.ok) {
      const paths = r.errors.map((e) => e.path);
      expect(paths).toContain("/status");
      expect(paths).toContain("/builds/0/os");
    }
  });

  it("rejects placeholder or uppercase hashes", () => {
    const m = valid();
    m.assets_required[0]!.accept[0]!.sha1 = "[TBD]";
    expect(validateManifest(m).ok).toBe(false);
    m.assets_required[0]!.accept[0]!.sha1 = "A".repeat(40);
    expect(validateManifest(m).ok).toBe(false);
  });

  it("rejects links to game files", () => {
    const m = valid();
    m.links.homepage = "https://example.com/files/game.z64";
    const r = validateManifest(m);
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.errors[0]).toMatchObject({ path: "/links/homepage" });
  });

  it("rejects duplicate builds and builds without a launch target", () => {
    const m = valid();
    m.builds.push({ ...m.builds[0]!, launch: {} } as (typeof m.builds)[number]);
    const r = validateManifest(m);
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.errors.map((e) => e.message)).toEqual(
        expect.arrayContaining([
          "duplicate build windows/x86_64",
          "needs launch.exe or launch.app_bundle",
        ]),
      );
    }
  });

  it("accepts an archive inside an archive", () => {
    const m = valid();
    const builds = m.builds as Array<Record<string, unknown>>;
    builds[2]!.nested = { file: "Example-*.dmg", archive: "dmg" };
    expect(validateManifest(m)).toMatchObject({ ok: true });
  });

  it("rejects a nested archive the outer one can't hold", () => {
    const m = valid();
    const builds = m.builds as Array<Record<string, unknown>>;
    builds[2]!.archive = "dmg";
    builds[2]!.nested = { file: "Example.zip", archive: "zip" };
    builds[1]!.nested = { file: "Example.apk", archive: "apk" };
    const r = validateManifest(m);
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.errors.map((e) => e.path)).toEqual(
        expect.arrayContaining(["/builds/2/archive", "/builds/1/nested/archive"]),
      );
    }
  });

  describe("url_list sources", () => {
    const hosts = { urlListHosts: ["downloads.example.org"] };
    const listed = () => ({
      ...valid(),
      source: {
        type: "url_list",
        version: "3.1",
        released: "2026-09-30",
        release_url: "https://downloads.example.org/news/3.1",
        hosts: ["downloads.example.org"],
      },
      builds: [
        {
          os: "linux",
          arch: "x86_64",
          asset: "example-3.1-linux.tar.xz",
          url: "https://downloads.example.org/3.1/example-3.1-linux.tar.xz",
          sha256: "c".repeat(64),
          size: 1234,
          archive: "tar.xz",
          launch: { exe: "example" },
        } as Record<string, unknown>,
      ],
    });
    const paths = (m: unknown, opts = hosts) => {
      const r = validateManifest(m, opts);
      return r.ok ? [] : r.errors.map((e) => e.path);
    };

    it("accepts a version with its files", () => {
      expect(validateManifest(listed(), hosts)).toMatchObject({ ok: true });
    });

    it("only allows reviewed hosts", () => {
      expect(paths(listed(), { urlListHosts: [] })).toEqual(["/source/hosts/0"]);
    });

    it("needs the version fields and each file's url, sha256 and size", () => {
      const m = listed();
      const src = m.source as Record<string, unknown>;
      src.version = undefined;
      src.repo = "example-org/example-port";
      m.builds[0]!.sha256 = undefined;
      expect(paths(m)).toEqual(
        expect.arrayContaining(["/source/repo", "/source/version", "/builds/0/sha256"]),
      );
    });

    it("checks each url against the hosts and the asset name", () => {
      const m = listed();
      m.builds[0]!.url = "https://elsewhere.example.net/example-3.1-linux.tar.xz";
      m.builds[0]!.asset = "other.tar.xz";
      expect(paths(m)).toEqual(expect.arrayContaining(["/builds/0/url", "/builds/0/asset"]));
      m.builds[0]!.url = "http://downloads.example.org/3.1/other.tar.xz";
      expect(paths(m)).toEqual(["/builds/0/url"]);
    });

    it("keeps url_list fields off GitHub manifests", () => {
      const m = valid();
      (m.source as Record<string, unknown>).version = "1";
      (m.builds[0] as Record<string, unknown>).sha256 = "c".repeat(64);
      expect(paths(m)).toEqual(["/source/version", "/builds/0/sha256"]);
    });
  });

  describe("game_files", () => {
    const withGameFiles = (gameFiles: Record<string, unknown>) => ({
      ...valid(),
      game_files: {
        label: "Example Game",
        stores: { steam: [1000], gog: ["2000"], epic: ["ExampleApp"], itch: [3000] },
        files: ["data.pak", "music/"],
        dest: { macos: "{install}/Example.app/Contents/Resources/" },
        ...gameFiles,
      },
    });

    it("accepts store ids, files and a destination", () => {
      expect(validateManifest(withGameFiles({}))).toMatchObject({ ok: true });
    });

    it("keeps files inside the game's folder", () => {
      const r = validateManifest(
        withGameFiles({ files: ["../secret", "/etc/x", "a\\b", "*.dat", "C:/x", "ok/file"] }),
      );
      expect(r.ok).toBe(false);
      if (!r.ok) {
        expect(r.errors.map((e) => e.path)).toEqual(
          [0, 1, 2, 3, 4].map((i) => `/game_files/files/${i}`),
        );
      }
    });

    it("needs a placeholder at the start of each destination", () => {
      const r = validateManifest(
        withGameFiles({ dest: { windows: "C:/Games/", linux: "{install}/../x/" } }),
      );
      expect(r.ok).toBe(false);
      if (!r.ok) {
        expect(r.errors.map((e) => e.path)).toEqual([
          "/game_files/dest/windows",
          "/game_files/dest/linux",
        ]);
      }
    });

    it("rejects a store id listed twice and unknown stores", () => {
      expect(validateManifest(withGameFiles({ stores: { steam: [1000, 1000] } }))).toMatchObject({
        ok: false,
        errors: [{ path: "/game_files/stores/steam" }],
      });
      expect(validateManifest(withGameFiles({ stores: { itch: ["x"] } })).ok).toBe(false);
    });
  });

  it("rejects unknown fields", () => {
    expect(validateManifest({ ...valid(), rom_url: "x" }).ok).toBe(false);
  });

  it("warns when redistribution is claimed", () => {
    const r = validateManifest({ ...valid(), redistribution: true });
    expect(r).toMatchObject({ ok: true, warnings: [{ path: "/redistribution" }] });
  });
});

describe("schemas/manifest.v1.json", () => {
  it("is up to date (run `pnpm schema:gen`)", () => {
    const file = JSON.parse(
      readFileSync(new URL("../schemas/manifest.v1.json", import.meta.url), "utf8"),
    );
    const { $schema, ...rest } = file;
    expect($schema).toBe("https://json-schema.org/draft/2020-12/schema");
    expect(rest).toEqual(JSON.parse(JSON.stringify(ManifestSchema)));
  });
});
