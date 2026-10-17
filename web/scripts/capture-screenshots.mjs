// Recaptures the desktop app screenshots on the landing page (src/assets/screenshots/).
// The app's UI runs in a browser in its mock mode, with the mock's synthetic catalog swapped
// for the real published one, so the games, stars, statuses and releases are real:
//   1. in ../client:
//      `pnpm --filter @portside/ui dev` (serves http://localhost:5173)
//   2. here: `pnpm screenshots` (Playwright with its Chromium; `pnpm exec playwright install
//      chromium` once if it has none).
// PORTSIDE_UI_URL points it at another address. The catalog comes from PORTSIDE_CATALOG_DIR (a
// locally built catalog, like ../catalog/dist) or else PORTSIDE_CATALOG_URL (default: the
// published catalog). Installs, play and the people in Community and chat stay simulated.
// Covers are the catalog's own image links, loaded live as in the app (website exception in
// docs/legal/policy.md, rule 6). A cover that fails to load falls back to the app's gradient,
// so the script lists those at the end: rerun from a network that reaches them.
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const ui = process.env.PORTSIDE_UI_URL ?? "http://localhost:5173";
const out = fileURLToPath(new URL("../src/assets/screenshots/", import.meta.url));
const catalogDir = process.env.PORTSIDE_CATALOG_DIR;
const catalogUrl = (
  process.env.PORTSIDE_CATALOG_URL ?? "https://erickbarrera.github.io/portside"
).replace(/\/+$/, "");

async function catalogFile(path) {
  if (catalogDir) return JSON.parse(await readFile(join(catalogDir, path), "utf8"));
  const res = await fetch(`${catalogUrl}/${path}`);
  if (!res.ok) throw new Error(`${catalogUrl}/${path}: HTTP ${res.status}`);
  return res.json();
}

// Same order as the app's catalog (portside-catalog search.rs): most stars first.
const all = (await catalogFile("v1/snapshot.json")).ports.sort((a, b) => (b.stars ?? -1) - (a.stars ?? -1) || a.id.localeCompare(b.id));
// The mock lists every row it has without paging, so it gets the top of the list plus every
// port with a manifest; the port count it reports is patched to the real total below.
const ports = all.filter((p, i) => i < 60 || p.os_status.length > 0);
const detailIds = ports.map((p) => p.id);
const details = Object.fromEntries(
  await Promise.all(
    detailIds.map(async (id) => [id, await catalogFile(`v1/ports/${id}.json`)]),
  ),
);
// Stands in for the mock's fixtures module (client/ui/src/lib/mock/fixtures.ts).
const fixtures = `export const ports = ${JSON.stringify(ports)};
const details = ${JSON.stringify(details)};
export function portDetail(id) { return details[id] ?? null; }
`;

const browser = await chromium.launch();
const page = await browser.newPage({
  viewport: { width: 1280, height: 800 },
  deviceScaleFactor: 2,
  colorScheme: "dark",
});
const missing = new Set();
page.on("requestfailed", (req) => {
  if (req.resourceType() === "image" && !req.url().startsWith("data:")) missing.add(req.url());
});
page.on("response", (res) => {
  if (res.request().resourceType() === "image" && res.status() >= 400) missing.add(res.url());
});
await page.route(/\/src\/lib\/mock\/fixtures\.ts(\?.*)?$/, (route) =>
  route.fulfill({ contentType: "text/javascript", body: fixtures }),
);
await page.route(/\/src\/lib\/mock\/ipc\.ts(\?.*)?$/, async (route) => {
  const res = await route.fetch();
  let body = await res.text();
  // The mock's install plan has made-up values that would contradict the real releases.
  for (const [from, to] of [
    ["port_count: ports.length", `port_count: ${all.length}`],
    ['version: "v1.2.0"', "version: port.latest_releases[0].tag"],
    ["repo: `example/${id}`", "repo: port.repo ?? id"],
  ]) {
    if (!body.includes(from)) throw new Error(`mock ipc.ts no longer has ${from}`);
    body = body.replace(from, to);
  }
  await route.fulfill({ response: res, body });
});
const shot = async (name) => {
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(900);
  await page.mouse.move(1270, 790); // no hover states
  await page.screenshot({ path: `${out}${name}.png` });
  console.log(`wrote ${name}.png`);
};
const title = (id) => {
  const port = all.find((p) => p.id === id);
  if (!port) throw new Error(`${id} is not in the catalog`);
  return port.title;
};
const install = async (id) => {
  await page.getByRole("link", { name: "Discover" }).click();
  await page.getByText(title(id), { exact: true }).first().click();
  await page.getByRole("button", { name: "Install" }).click();
  // macOS asks before installing an unsigned app.
  const anyway = page.getByRole("button", { name: "Install anyway" });
  await anyway.waitFor({ timeout: 2000 }).catch(() => {});
  if (await anyway.count()) await anyway.click();
  await page.getByRole("button", { name: "Play" }).waitFor({ timeout: 30_000 });
};

await page.goto(`${ui}/discover?mock-os=macos`);
// A few installed games first, so the sidebar's Installed list shows in every screenshot.
for (const id of [
  "harbourmasters-shipwright",
  "openttd-openttd",
  "diasurgical-devilutionx",
  "openrct2-openrct2",
]) {
  await install(id);
}
await page.getByRole("button", { name: "Play" }).click();
await page.mouse.move(800, 400);
await page.mouse.wheel(0, -5000); // back to the top, cover included
await shot("game-page");
await page.getByRole("link", { name: "Discover" }).click();
await page.mouse.wheel(0, -5000);
await shot("discover");
await page.getByRole("link", { name: "Library" }).click();
await shot("library");
await page.getByRole("link", { name: "Community" }).click();
await shot("community");
await page.getByRole("link", { name: /Friends/ }).click();
await page.getByRole("heading", { name: "Friends", exact: true }).waitFor();
await page.getByText("Example Ada").first().click(); // the mock's made-up friend
await shot("chat");
await browser.close();
if (missing.size > 0) {
  console.warn(`\n${missing.size} game image(s) did not load and show the gradient instead:`);
  for (const url of missing) console.warn(`  ${url}`);
  process.exitCode = 1;
}
