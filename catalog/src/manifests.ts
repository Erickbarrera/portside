/**
 * Load port manifests from a folder (`manifests/`, files `ports/<id>.yaml`) and validate them.
 * Spec: docs/specs/manifest-format.md. An invalid file fails the catalog build (src/cli.ts),
 * so a bad manifest never reaches the published catalog.
 */
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { readFile, readdir } from "node:fs/promises";
import { basename, extname, join } from "node:path";
import { parse as parseYaml } from "yaml";
import { type Manifest, type ManifestIssue, validateManifest } from "./schema/manifest.js";

export interface LoadedManifest {
  file: string;
  manifest: Manifest;
  rev: string;
  warnings: ManifestIssue[];
}

export interface InvalidManifest {
  file: string;
  errors: ManifestIssue[];
}

const MAX_BYTES = 256 * 1024;

export async function loadManifests(
  dir: string,
): Promise<{ valid: LoadedManifest[]; invalid: InvalidManifest[] }> {
  const root = existsSync(join(dir, "ports")) ? join(dir, "ports") : dir;
  const files = (await readdir(root))
    .filter((f) => [".yaml", ".yml", ".json"].includes(extname(f)))
    .sort();
  const valid: LoadedManifest[] = [];
  const invalid: InvalidManifest[] = [];
  for (const file of files) {
    const text = await readFile(join(root, file), "utf8");
    if (text.length > MAX_BYTES) {
      invalid.push({ file, errors: [{ path: "/", message: `larger than ${MAX_BYTES} bytes` }] });
      continue;
    }
    let data: unknown;
    try {
      data = extname(file) === ".json" ? JSON.parse(text) : parseYaml(text, { maxAliasCount: 0 });
    } catch (err) {
      invalid.push({
        file,
        errors: [{ path: "/", message: `parse error: ${(err as Error).message}` }],
      });
      continue;
    }
    const r = validateManifest(data);
    if (!r.ok) {
      invalid.push({ file, errors: r.errors });
      continue;
    }
    if (r.manifest.id !== basename(file, extname(file))) {
      invalid.push({ file, errors: [{ path: "/id", message: "must match the file name" }] });
      continue;
    }
    const rev = createHash("sha256").update(text).digest("hex").slice(0, 16);
    valid.push({ file, manifest: r.manifest, rev, warnings: r.warnings });
  }
  return { valid, invalid };
}
