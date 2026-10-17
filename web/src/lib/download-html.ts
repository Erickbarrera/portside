// The download section as an HTML string, for the build and the browser alike (like
// port-card.ts), so a newer release found in the browser renders the same way.
import {
  type AppRelease,
  type Download,
  OS_LABEL,
  OS_ORDER,
  type Os,
  RELEASES_PAGE,
  formatSize,
} from "./releases";

const esc = (s: string) =>
  s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

/** How to open an unsigned build the first time. */
export const FIRST_RUN: Record<Os, string> = {
  macos:
    "The alpha isn't signed yet. Open Portside once, close the warning, then choose Open Anyway in System Settings → Privacy & Security.",
  windows:
    "The alpha isn't signed yet. If SmartScreen warns about an unknown publisher, choose More info → Run anyway.",
  linux: "Make the AppImage executable (chmod +x) and run it, or install the .deb or .rpm.",
};

export function releaseLabel(r: AppRelease): string {
  const date = new Date(r.published_at).toLocaleDateString("en", {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
  return `${r.prerelease ? "Alpha " : ""}${r.version} · ${date}`;
}

/** The big button's file on this OS. */
export function primaryFor(r: AppRelease, os: Os | null): Download | null {
  if (!os) return null;
  return r.downloads.find((d) => d.os === os && d.primary) ?? null;
}

function fileRow(d: Download): string {
  return `<li><a class="dl-file" href="${esc(d.url)}"><span class="dl-label">${esc(d.label)}</span><span class="dl-note">${esc(d.note)} · ${formatSize(d.size)}</span></a></li>`;
}

/** One card per OS that has files; the detected OS first and marked. */
export function downloadsHtml(r: AppRelease | null, os: Os | null): string {
  if (!r) {
    return `<p class="dl-empty">Downloads are coming with the public alpha. Follow along on <a href="${RELEASES_PAGE}">GitHub</a>.</p>`;
  }
  const order = os ? [os, ...OS_ORDER.filter((o) => o !== os)] : OS_ORDER;
  const cards = order
    .map((o) => {
      const files = r.downloads.filter((d) => d.os === o);
      if (files.length === 0) return "";
      const mine = o === os;
      return `<li class="dl-card${mine ? " is-mine" : ""}"><h3>${OS_LABEL[o]}${mine ? ' <span class="dl-yours">Your system</span>' : ""}</h3><ul class="dl-files">${files.map(fileRow).join("")}</ul><p class="dl-tip">${esc(FIRST_RUN[o])}</p></li>`;
    })
    .join("");
  return `<p class="dl-meta">${esc(releaseLabel(r))} · <a href="${esc(r.html_url)}">Release notes</a></p><ul class="dl-cards">${cards}</ul>`;
}
