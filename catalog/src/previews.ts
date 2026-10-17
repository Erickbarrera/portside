/**
 * The preview image each port's own team set for its GitHub repo ("social preview"), used
 * as the cover when the upstream list has no screenshot (docs/legal/policy.md rule 6).
 * Repos without a custom image are left out: GitHub's generated card is just the repo name.
 */

/** Repos per GraphQL request. */
const BATCH = 50;
const REPO = /^([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)$/;
const IMAGE_HOST = "https://repository-images.githubusercontent.com/";

export interface PreviewOptions {
  /** GraphQL needs a token; without one nothing is fetched. */
  token: string;
  apiUrl?: string;
  fetchFn?: typeof fetch;
}

/** The GraphQL query for one batch, with a field alias per repo. */
export function previewQuery(repos: string[]): string {
  const fields = repos.map((r, i) => {
    const m = REPO.exec(r);
    if (!m) throw new Error(`not a repo: ${r}`);
    return `r${i}: repository(owner: ${JSON.stringify(m[1])}, name: ${JSON.stringify(m[2])}) { usesCustomOpenGraphImage openGraphImageUrl }`;
  });
  return `query {\n${fields.join("\n")}\n}`;
}

interface RepoImage {
  usesCustomOpenGraphImage?: boolean;
  openGraphImageUrl?: string;
}

/** Custom preview images from a batch's `data`, by repo (lowercase). */
export function readPreviews(
  repos: string[],
  data: Record<string, RepoImage | null> | null | undefined,
): Map<string, string> {
  const out = new Map<string, string>();
  repos.forEach((repo, i) => {
    const r = data?.[`r${i}`];
    if (r?.usesCustomOpenGraphImage && r.openGraphImageUrl?.startsWith(IMAGE_HOST)) {
      out.set(repo.toLowerCase(), r.openGraphImageUrl);
    }
  });
  return out;
}

/**
 * Custom preview images for `repos`, by repo (lowercase). Missing or renamed repos are
 * skipped. Throws when a request fails, so the caller can keep the published images.
 */
export async function fetchPreviews(
  repos: string[],
  opts: PreviewOptions,
): Promise<Map<string, string>> {
  const api = opts.apiUrl ?? "https://api.github.com";
  const doFetch = opts.fetchFn ?? fetch;
  const unique = [...new Set(repos.filter((r) => REPO.test(r)))].sort();
  const out = new Map<string, string>();
  for (let i = 0; i < unique.length; i += BATCH) {
    const batch = unique.slice(i, i + BATCH);
    const res = await doFetch(`${api}/graphql`, {
      method: "POST",
      headers: {
        authorization: `Bearer ${opts.token}`,
        "content-type": "application/json",
        "user-agent": "portside-catalog",
      },
      body: JSON.stringify({ query: previewQuery(batch) }),
      signal: AbortSignal.timeout(60_000),
    });
    if (!res.ok) throw new Error(`GitHub GraphQL: HTTP ${res.status}`);
    const body = (await res.json()) as {
      data?: Record<string, RepoImage | null> | null;
      errors?: { type?: string; message?: string }[];
    };
    // NOT_FOUND errors (deleted or renamed repos) come with partial data; anything else fails.
    const other = (body.errors ?? []).filter((e) => e.type !== "NOT_FOUND");
    if (other.length > 0 || !body.data) {
      throw new Error(`GitHub GraphQL: ${other[0]?.message ?? "no data"}`);
    }
    for (const [repo, url] of readPreviews(batch, body.data)) out.set(repo, url);
  }
  return out;
}
