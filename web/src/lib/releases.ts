// Desktop app downloads: the newest published release of this repository on GitHub,
// pre-releases (alpha) included. Read at build time and again in the browser, so the download
// button follows a new release without a site rebuild. Shared by both (no Astro imports).

export const APP_REPO = "Erickbarrera/portside";
export const RELEASES_PAGE = `https://github.com/${APP_REPO}/releases`;

export type Os = "macos" | "windows" | "linux";

export interface Download {
  os: Os;
  /** What it is, e.g. "Installer (.exe)". */
  label: string;
  /** Short note under the label, e.g. "Apple Silicon and Intel". */
  note: string;
  name: string;
  url: string;
  size: number;
  /** The file the big button offers on this OS. */
  primary: boolean;
}

export interface AppRelease {
  tag: string;
  version: string;
  name: string;
  prerelease: boolean;
  published_at: string;
  html_url: string;
  downloads: Download[];
}

interface GhAsset {
  name: string;
  browser_download_url: string;
  size: number;
}

interface GhRelease {
  tag_name: string;
  name: string | null;
  draft: boolean;
  prerelease: boolean;
  published_at: string | null;
  html_url: string;
  assets: GhAsset[];
}

export const OS_LABEL: Record<Os, string> = {
  macos: "macOS",
  windows: "Windows",
  linux: "Linux",
};
export const OS_ORDER: Os[] = ["macos", "windows", "linux"];

// File name → download. Order within an OS is the order shown; the first rule per OS is the
// one the big button offers. Updater archives and signatures are left out.
const KINDS: { test: RegExp; os: Os; label: string; note: string }[] = [
  { test: /\.dmg$/i, os: "macos", label: "Disk image (.dmg)", note: "Apple Silicon and Intel" },
  { test: /-setup\.exe$/i, os: "windows", label: "Installer (.exe)", note: "Windows 10 and 11, x64" },
  { test: /\.msi$/i, os: "windows", label: "MSI package (.msi)", note: "For managed installs" },
  { test: /\.AppImage$/i, os: "linux", label: "AppImage", note: "Any distribution, Steam Deck" },
  { test: /\.deb$/i, os: "linux", label: "Debian package (.deb)", note: "Debian, Ubuntu, Mint" },
  { test: /\.rpm$/i, os: "linux", label: "RPM package (.rpm)", note: "Fedora, openSUSE" },
];

export function toDownloads(assets: GhAsset[]): Download[] {
  const out: Download[] = [];
  for (const os of OS_ORDER) {
    let first = true;
    for (const kind of KINDS) {
      if (kind.os !== os) continue;
      const asset = assets.find((a) => kind.test.test(a.name));
      if (!asset) continue;
      out.push({
        os,
        label: kind.label,
        note: kind.note,
        name: asset.name,
        url: asset.browser_download_url,
        size: asset.size,
        primary: first,
      });
      first = false;
    }
  }
  return out;
}

/** Newest published release with at least one download, or null. */
export function pickRelease(list: GhRelease[]): AppRelease | null {
  const published = list
    .filter((r) => !r.draft && r.published_at)
    .sort((a, b) => Date.parse(b.published_at ?? "") - Date.parse(a.published_at ?? ""));
  for (const r of published) {
    const downloads = toDownloads(r.assets);
    if (downloads.length === 0) continue;
    return {
      tag: r.tag_name,
      version: r.tag_name.replace(/^client-v/, ""),
      name: r.name || r.tag_name,
      prerelease: r.prerelease,
      published_at: r.published_at as string,
      html_url: r.html_url,
      downloads,
    };
  }
  return null;
}

/** Reads the releases from the GitHub API. Throws on HTTP errors. */
export async function fetchAppRelease(token?: string): Promise<AppRelease | null> {
  const headers: Record<string, string> = {
    accept: "application/vnd.github+json",
    "x-github-api-version": "2022-11-28",
  };
  if (token) headers.authorization = `Bearer ${token}`;
  const res = await fetch(`https://api.github.com/repos/${APP_REPO}/releases?per_page=10`, {
    headers,
    signal: AbortSignal.timeout(15_000),
  });
  if (!res.ok) throw new Error(`GitHub returned HTTP ${res.status} for ${APP_REPO} releases`);
  return pickRelease((await res.json()) as GhRelease[]);
}

/** The visitor's desktop OS, or null for phones, tablets and anything unrecognised. */
export function detectOs(nav: {
  userAgent: string;
  platform?: string;
  userAgentData?: { platform?: string; mobile?: boolean };
}): Os | null {
  const uaData = nav.userAgentData;
  if (uaData?.mobile) return null;
  const hint = `${uaData?.platform ?? ""} ${nav.platform ?? ""} ${nav.userAgent}`.toLowerCase();
  // iPadOS reports a Mac user agent; touch support gives it away in the browser (see caller).
  if (/android|iphone|ipad|ipod|cros/.test(hint)) return null;
  if (/mac/.test(hint)) return "macos";
  if (/win/.test(hint)) return "windows";
  if (/linux|x11|freebsd/.test(hint)) return "linux";
  return null;
}

export function formatSize(bytes: number): string {
  return `${Math.round(bytes / 1e5) / 10} MB`;
}
