import { describe, expect, it } from "vitest";
import { fetchReleases, globToRegExp, urlListRelease } from "../src/releases.js";
import type { Manifest } from "../src/schema/manifest.js";

// Synthetic manifest and releases: invented project, no real repos or files.
const manifest = (over: Partial<Manifest["source"]> = {}): Manifest => ({
  schema: 1,
  id: "example-port",
  game_key: "example-game",
  status: "playable",
  source: { type: "github_releases", repo: "example-org/example-port", channel: "stable", ...over },
  builds: [
    {
      os: "windows",
      arch: "x86_64",
      asset: "Example-*-win64.zip",
      archive: "zip",
      launch: { exe: "example.exe" },
    },
    {
      os: "macos",
      arch: "universal",
      asset: "Example-*-mac.zip",
      archive: "zip",
      launch: { app_bundle: "Example.app" },
    },
  ],
});

const asset = (name: string, digest: string | null = null) => ({
  name,
  browser_download_url: `https://github.com/example-org/example-port/releases/download/x/${name}`,
  size: 100,
  digest,
});

const ghReleases = [
  {
    id: 3,
    tag_name: "v2.0.0-rc1",
    name: "RC",
    draft: false,
    prerelease: true,
    published_at: "2026-10-03T00:00:00Z",
    html_url: "https://github.com/example-org/example-port/releases/tag/v2.0.0-rc1",
    assets: [asset("Example-2.0.0-rc1-win64.zip")],
  },
  {
    id: 2,
    tag_name: "v1.1.0",
    name: "",
    draft: false,
    prerelease: false,
    published_at: "2026-10-02T00:00:00Z",
    html_url: "https://github.com/example-org/example-port/releases/tag/v1.1.0",
    assets: [
      asset("Example-1.1.0-win64.zip", `sha256:${"a".repeat(64)}`),
      asset("Example-1.1.0-mac.zip"),
      asset("Example-1.1.0-mac.zip.sha256"),
    ],
  },
  {
    id: 4,
    tag_name: "v1.2.0",
    name: null,
    draft: true,
    prerelease: false,
    published_at: null,
    html_url: "https://github.com/example-org/example-port/releases/tag/untagged",
    assets: [],
  },
];

const fakeFetch = (status = 200) =>
  (async (url: string | URL | Request, init?: RequestInit) => {
    calls.push({ url: String(url), auth: new Headers(init?.headers).get("authorization") });
    return new Response(JSON.stringify(ghReleases), { status });
  }) as typeof fetch;
let calls: { url: string; auth: string | null }[] = [];

describe("globToRegExp", () => {
  it("matches whole names with * ? and classes", () => {
    expect(globToRegExp("A-*-win64.zip").test("A-1.0-win64.zip")).toBe(true);
    expect(globToRegExp("A-*-win64.zip").test("A-1.0-win64.zip.sha256")).toBe(false);
    expect(globToRegExp("v?.zip").test("v1.zip")).toBe(true);
    expect(globToRegExp("[ab].zip").test("b.zip")).toBe(true);
    expect(globToRegExp("[!ab].zip").test("b.zip")).toBe(false);
    expect(globToRegExp("a+b(1).zip").test("a+b(1).zip")).toBe(true);
  });
});

describe("fetchReleases", () => {
  it("stable: skips drafts and prereleases, maps assets and digests", async () => {
    calls = [];
    const releases = await fetchReleases(manifest(), { token: "t", fetch: fakeFetch() });
    expect(calls).toEqual([
      {
        url: "https://api.github.com/repos/example-org/example-port/releases?per_page=30",
        auth: "Bearer t",
      },
    ]);
    expect(releases.map((r) => r.tag)).toEqual(["v1.1.0"]);
    const [r] = releases;
    expect(r?.name).toBeNull();
    expect(r?.id).toBe("2");
    expect(r?.assets).toEqual([
      {
        os: "windows",
        arch: "x86_64",
        name: "Example-1.1.0-win64.zip",
        url: expect.stringContaining("win64.zip"),
        size: 100,
        sha256: "a".repeat(64),
      },
      {
        os: "macos",
        arch: "universal",
        name: "Example-1.1.0-mac.zip",
        url: expect.stringContaining("mac.zip"),
        size: 100,
        sha256: null,
      },
    ]);
  });

  it("prerelease and tag_pattern channels", async () => {
    const pre = await fetchReleases(manifest({ channel: "prerelease" }), { fetch: fakeFetch() });
    expect(pre.map((r) => r.tag)).toEqual(["v2.0.0-rc1", "v1.1.0"]);
    const tagged = await fetchReleases(
      manifest({ channel: "tag_pattern", tag_pattern: "-rc\\d+$" }),
      {
        fetch: fakeFetch(),
      },
    );
    expect(tagged.map((r) => r.tag)).toEqual(["v2.0.0-rc1"]);
  });

  it("throws on HTTP errors and ignores non-GitHub sources", async () => {
    await expect(fetchReleases(manifest(), { fetch: fakeFetch(403) })).rejects.toThrow(/HTTP 403/);
    expect(
      await fetchReleases(manifest({ type: "github_actions_artifact" }), { fetch: fakeFetch() }),
    ).toEqual([]);
  });
});

describe("urlListRelease", () => {
  const listed = (): Manifest => ({
    ...manifest(),
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
        sha256: "b".repeat(64),
        size: 1234,
        archive: "tar.xz",
        launch: { exe: "example" },
      },
    ],
  });

  it("makes the one release from the manifest without the network", async () => {
    calls = [];
    const releases = await fetchReleases(listed(), { fetch: fakeFetch() });
    expect(calls).toEqual([]);
    expect(releases).toEqual([urlListRelease(listed())]);
    expect(releases[0]).toEqual({
      id: "url_list:3.1",
      port_id: "example-port",
      tag: "3.1",
      name: "3.1",
      published_at: "2026-09-30T00:00:00.000Z",
      prerelease: false,
      revoked: false,
      html_url: "https://downloads.example.org/news/3.1",
      assets: [
        {
          os: "linux",
          arch: "x86_64",
          name: "example-3.1-linux.tar.xz",
          url: "https://downloads.example.org/3.1/example-3.1-linux.tar.xz",
          size: 1234,
          sha256: "b".repeat(64),
        },
      ],
    });
  });

  it("keeps a full timestamp", () => {
    const m = listed();
    m.source.released = "2026-09-30T18:26:00+02:00";
    expect(urlListRelease(m).published_at).toBe("2026-09-30T16:26:00.000Z");
  });
});
