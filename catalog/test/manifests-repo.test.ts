// The real manifests in ../../manifests (docs/specs/manifest-format.md, "Where manifests
// live"). Checked offline: schema and rules here. Asset globs are checked against upstream
// releases when the catalog is built (src/cli.ts warns about builds no file matches).
import { readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { loadManifests } from "../src/manifests.js";

const dir = fileURLToPath(new URL("../../manifests", import.meta.url));

describe("manifests/ports", () => {
  it("every file is a valid manifest", async () => {
    const files = readdirSync(`${dir}/ports`);
    const { valid, invalid } = await loadManifests(dir);
    expect(invalid).toEqual([]);
    expect(valid).toHaveLength(files.length);
    for (const v of valid) expect(v.warnings).toEqual([]);
  });
});
