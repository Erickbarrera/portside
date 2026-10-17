import type { PlayableStatus, PortSummary } from "./catalog";
import { coverBackground, coverTitle } from "./cover";
import { osLabel, statusLabel, typeLabel } from "./status";

// One port row as an HTML string. The catalog page renders the first page with it on the
// server and appends later pages (search, "Load more") with it in the browser, so both
// produce identical markup. Styles: `.port` and `.badge` in src/styles/global.css.

const esc = (s: string) =>
  s.replace(
    /[&<>"']/g,
    (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c] ?? c,
  );

const nf = new Intl.NumberFormat("en");

export function badgeHtml(status: PlayableStatus, prefix?: string): string {
  return (
    `<span class="badge s-${esc(status)}"><span class="dot" aria-hidden="true"></span>` +
    (prefix ? `<span class="prefix">${esc(prefix)}</span>` : "") +
    `${esc(statusLabel[status] ?? status)}</span>`
  );
}

// The port's picture from the catalog (`screenshot_url`), the same link the desktop app shows:
// loaded straight from where it is hosted, never copied (docs/legal/policy.md rule 6). Under it
// sits the generated cover the app uses, which shows when there is no picture or it fails to
// load (the inline handler drops the broken image; it has to be inline because a row built on
// the server can fail before any page script runs).
export function coverHtml(p: PortSummary): string {
  const url = p.screenshot_url && /^https:\/\//.test(p.screenshot_url) ? p.screenshot_url : null;
  const img = url
    ? `<img src="${esc(url)}" alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer" onerror="this.remove()">`
    : "";
  return (
    `<div class="cover" style="background:${esc(coverBackground(p.game_key))}">` +
    `<span class="name" aria-hidden="true">${esc(coverTitle(p.title))}</span>${img}</div>`
  );
}

export function portCardHtml(p: PortSummary): string {
  const meta = [`<span>${esc(typeLabel[p.type] ?? p.type)}</span>`];
  if (p.stars != null) meta.push(`<span>★ ${nf.format(p.stars)}</span>`);
  if (p.archived) meta.push(`<span class="archived">Archived</span>`);

  const status = [badgeHtml(p.status, "Overall")];
  if (p.os_status.length > 0) {
    const items = p.os_status.map((o) => `<li>${badgeHtml(o.status, osLabel[o.os] ?? o.os)}</li>`);
    status.push(`<ul class="os" aria-label="Status by operating system">${items.join("")}</ul>`);
  }
  if (p.play_url && /^https?:\/\//.test(p.play_url)) {
    status.push(`<a class="play" href="${esc(p.play_url)}" rel="noopener">Play in browser</a>`);
  }

  return (
    `<li class="port">${coverHtml(p)}<div class="main"><h2>${esc(p.title)}</h2>` +
    `<p class="meta">${meta.join("")}</p></div>` +
    `<div class="status">${status.join("")}</div></li>`
  );
}
