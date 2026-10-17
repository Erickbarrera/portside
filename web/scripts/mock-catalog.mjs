// Serves a synthetic static catalog (../docs/specs/static-catalog.md) so the site can be
// developed offline. Usage: node scripts/mock-catalog.mjs [port]   (default 4010)
import { createServer } from "node:http";
import { ports } from "./fixtures.mjs";

const port = Number(process.argv[2] ?? process.env.MOCK_CATALOG_PORT ?? 4010);
const files = new Map([
  ["/v1/snapshot.json", { rev: 1, ports }],
  ["/v1/index.json", { schema: 1, rev: 1, generated_at: "2026-10-14T00:00:00Z", port_count: ports.length, content_hash: "mock" }],
  ...ports.map((p) => [`/v1/ports/${p.id}.json`, p]),
]);

createServer((req, res) => {
  const path = new URL(req.url ?? "/", "http://localhost").pathname;
  const body = files.get(path);
  // GitHub Pages also allows any origin.
  res.setHeader("access-control-allow-origin", "*");
  if (req.method !== "GET" || !body) {
    res.writeHead(404, { "content-type": "text/plain" });
    return res.end("Not found");
  }
  res.writeHead(200, { "content-type": "application/json" });
  res.end(JSON.stringify(body));
}).listen(port, () => {
  console.log(`Mock Portside catalog on http://localhost:${port}`);
});
