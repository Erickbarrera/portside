// Runs the mock catalog and `astro dev --host` together; Ctrl+C stops both.
// `--host` serves the site on your local network (Astro prints the Network URL). The browser
// reaches the mock through /mock-catalog on the site itself (proxied in astro.config.mjs), so
// it works from a phone on the same network too.
import { spawn } from "node:child_process";

const children = [
  spawn(process.execPath, ["scripts/mock-catalog.mjs"], { stdio: "inherit" }),
  spawn("astro", ["dev", "--host"], {
    stdio: "inherit",
    shell: true,
    env: {
      ...process.env,
      PORTSIDE_CATALOG_URL: "http://localhost:4010",
      PUBLIC_PORTSIDE_CATALOG_URL: "/mock-catalog",
    },
  }),
];

const stop = () => {
  for (const c of children) c.kill("SIGTERM");
  process.exit();
};
process.on("SIGINT", stop);
process.on("SIGTERM", stop);
for (const c of children) c.on("exit", stop);
