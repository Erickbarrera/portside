import { describe, expect, it } from "vitest";
import { fetchPreviews, previewQuery, readPreviews } from "../src/previews.js";

const img = (id: number) => `https://repository-images.githubusercontent.com/${id}/abc`;

describe("previews", () => {
  it("builds one aliased field per repo and refuses odd names", () => {
    const q = previewQuery(["example-org/one", "example.org/two_2"]);
    expect(q).toContain('r0: repository(owner: "example-org", name: "one")');
    expect(q).toContain('r1: repository(owner: "example.org", name: "two_2")');
    expect(() => previewQuery(['bad"/x'])).toThrow();
  });

  it("keeps only custom images on GitHub's image host", () => {
    const out = readPreviews(["A/one", "b/two", "c/three", "d/four"], {
      r0: { usesCustomOpenGraphImage: true, openGraphImageUrl: img(1) },
      r1: {
        usesCustomOpenGraphImage: false,
        openGraphImageUrl: "https://opengraph.githubassets.com/x",
      },
      r2: null,
      r3: { usesCustomOpenGraphImage: true, openGraphImageUrl: "https://evil.example/x.png" },
    });
    expect([...out]).toEqual([["a/one", img(1)]]);
  });

  it("batches requests, tolerates missing repos and fails on other errors", async () => {
    const repos = Array.from({ length: 120 }, (_, i) => `example-org/p${i}`);
    const bodies: string[] = [];
    const ok: typeof fetch = async (_url, init) => {
      bodies.push(String(init?.body));
      return new Response(
        JSON.stringify({
          data: {
            r0: { usesCustomOpenGraphImage: true, openGraphImageUrl: img(bodies.length) },
            r1: null,
          },
          errors: [{ type: "NOT_FOUND", message: "Could not resolve" }],
        }),
      );
    };
    const out = await fetchPreviews([...repos, "not a repo"], { token: "t", fetchFn: ok });
    expect(bodies).toHaveLength(3);
    expect(out.size).toBe(3);

    const failing: typeof fetch = async () =>
      new Response(JSON.stringify({ errors: [{ type: "RATE_LIMITED", message: "slow down" }] }));
    await expect(fetchPreviews(repos, { token: "t", fetchFn: failing })).rejects.toThrow(
      /slow down/,
    );
    const status: typeof fetch = async () => new Response("", { status: 502 });
    await expect(fetchPreviews(repos, { token: "t", fetchFn: status })).rejects.toThrow(/502/);
  });
});
