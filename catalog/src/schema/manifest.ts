/**
 * Port manifest, schema 1. Contract: docs/specs/manifest-format.md.
 * `pnpm schema:gen` writes the JSON Schema to schemas/manifest.v1.json for the manifests
 * repo's CI.
 */
import { type Static, Type } from "@sinclair/typebox";
import { TypeCompiler } from "@sinclair/typebox/compiler";
import {
  ArchSchema,
  ArchiveSchema,
  MultiplayerSchema,
  OsSchema,
  PlayableStatusSchema,
} from "./enums.js";

const Str = (opts: { maxLength?: number; minLength?: number; pattern?: string } = {}) =>
  Type.String({ maxLength: 1000, ...opts });
const Nullable = <T extends Parameters<typeof Type.Union>[0][number]>(t: T) =>
  Type.Union([t, Type.Null()]);

export const LaunchSpecSchema = Type.Object(
  {
    exe: Type.Optional(Str()),
    app_bundle: Type.Optional(Str()),
    args: Type.Optional(Type.Array(Str(), { maxItems: 64 })),
    cwd: Type.Optional(Str()),
    env: Type.Optional(Type.Record(Type.String(), Str())),
    chmod_x: Type.Optional(Type.Boolean()),
  },
  { additionalProperties: false },
);

/** An archive inside the release asset (manifest-format.md, "Archives"). */
export const NestedArchiveSchema = Type.Object(
  {
    file: Str({ minLength: 1, maxLength: 200 }),
    archive: ArchiveSchema,
  },
  { additionalProperties: false },
);

export const ManifestBuildSchema = Type.Object(
  {
    os: OsSchema,
    arch: ArchSchema,
    asset: Str({ minLength: 1, maxLength: 200 }),
    archive: ArchiveSchema,
    nested: Type.Optional(NestedArchiveSchema),
    strip_root: Type.Optional(Type.Boolean()),
    launch: Type.Optional(LaunchSpecSchema),
    checksum: Type.Optional(
      Type.Union([Type.Literal("upstream"), Type.Literal("attestation"), Type.Literal("none")]),
    ),
    unsigned: Type.Optional(Type.Boolean()),
    url: Type.Optional(Str({ pattern: "^https://", maxLength: 500 })),
    sha256: Type.Optional(Type.String({ pattern: "^[0-9a-f]{64}$" })),
    size: Type.Optional(Type.Integer({ minimum: 1 })),
    android_package: Type.Optional(Str({ pattern: "^[A-Za-z][\\w]*(\\.[A-Za-z][\\w]*)+$" })),
  },
  { additionalProperties: false },
);

export const AssetAcceptSchema = Type.Object(
  {
    sha1: Type.Optional(Type.String({ pattern: "^[0-9a-f]{40}$" })),
    sha256: Type.Optional(Type.String({ pattern: "^[0-9a-f]{64}$" })),
    name: Str({ minLength: 1, maxLength: 200 }),
  },
  { additionalProperties: false },
);

export const AssetRequirementSchema = Type.Object(
  {
    id: Str({ pattern: "^[a-z0-9_-]+$", maxLength: 64 }),
    label: Str({ minLength: 1, maxLength: 300 }),
    accept: Type.Array(AssetAcceptSchema, { minItems: 1 }),
    formats: Type.Optional(Type.Array(Str({ maxLength: 16 }))),
    normalize: Type.Optional(Nullable(Str({ maxLength: 64 }))),
    import: Type.Optional(
      Type.Object(
        {
          mode: Type.Union([
            Type.Literal("copy"),
            Type.Literal("extract_with_port"),
            Type.Literal("pick_folder"),
          ]),
          dest: Type.Optional(Str()),
          run: Type.Optional(Nullable(Str())),
        },
        { additionalProperties: false },
      ),
    ),
  },
  { additionalProperties: false },
);

export const ManifestSavesSchema = Type.Object(
  {
    root: Type.Object(
      {
        windows: Type.Optional(Str()),
        macos: Type.Optional(Str()),
        linux: Type.Optional(Str()),
        android: Type.Optional(Str()),
      },
      { additionalProperties: false },
    ),
    include: Type.Optional(Type.Array(Str())),
    exclude: Type.Optional(Type.Array(Str())),
    portable: Type.Optional(Type.Boolean()),
    tied_to_version: Type.Optional(Type.Boolean()),
    converter: Type.Optional(Nullable(Str())),
  },
  { additionalProperties: false },
);

/** Files from the player's own copy of the game (manifest-format.md, "game_files"). */
export const GameFilesSchema = Type.Object(
  {
    label: Str({ minLength: 1, maxLength: 200 }),
    stores: Type.Optional(
      Type.Object(
        {
          steam: Type.Optional(
            Type.Array(Type.Integer({ minimum: 1 }), { minItems: 1, maxItems: 8 }),
          ),
          gog: Type.Optional(
            Type.Array(Str({ pattern: "^[0-9]+$", maxLength: 20 }), { minItems: 1, maxItems: 8 }),
          ),
          epic: Type.Optional(
            Type.Array(Str({ pattern: "^[A-Za-z0-9_.-]+$", maxLength: 100 }), {
              minItems: 1,
              maxItems: 8,
            }),
          ),
        },
        { additionalProperties: false },
      ),
    ),
    files: Type.Array(Str({ minLength: 1, maxLength: 300 }), { minItems: 1, maxItems: 32 }),
    dest: Type.Optional(
      Type.Object(
        {
          windows: Type.Optional(Str()),
          macos: Type.Optional(Str()),
          linux: Type.Optional(Str()),
        },
        { additionalProperties: false },
      ),
    ),
  },
  { additionalProperties: false },
);

export const ManifestSchema = Type.Object(
  {
    schema: Type.Literal(1),
    id: Str({ pattern: "^[a-z0-9-]+$", minLength: 1, maxLength: 200 }),
    game_key: Str({ pattern: "^[a-z0-9-]+$", minLength: 1, maxLength: 200 }),
    display_name: Type.Optional(Str({ maxLength: 200 })),
    status: PlayableStatusSchema,
    status_notes: Type.Optional(Str({ maxLength: 2000 })),
    source: Type.Object(
      {
        type: Type.Union([
          Type.Literal("github_releases"),
          Type.Literal("github_actions_artifact"),
          Type.Literal("url_list"),
        ]),
        repo: Type.Optional(Str({ pattern: "^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$" })),
        channel: Type.Optional(
          Type.Union([
            Type.Literal("stable"),
            Type.Literal("prerelease"),
            Type.Literal("tag_pattern"),
          ]),
        ),
        tag_pattern: Type.Optional(Nullable(Str())),
        min_version: Type.Optional(Str({ maxLength: 64 })),
        version: Type.Optional(Str({ pattern: "^[A-Za-z0-9][A-Za-z0-9._+-]*$", maxLength: 64 })),
        released: Type.Optional(
          Str({
            pattern: "^\\d{4}-\\d{2}-\\d{2}(T[0-9:.]+(Z|[+-]\\d{2}:\\d{2}))?$",
            maxLength: 40,
          }),
        ),
        release_url: Type.Optional(Str({ pattern: "^https://", maxLength: 500 })),
        hosts: Type.Optional(
          Type.Array(Str({ pattern: "^[a-z0-9.-]+$", maxLength: 253 }), {
            minItems: 1,
            maxItems: 8,
          }),
        ),
      },
      { additionalProperties: false },
    ),
    builds: Type.Array(ManifestBuildSchema, { minItems: 1, maxItems: 32 }),
    assets_required: Type.Optional(Type.Array(AssetRequirementSchema)),
    game_files: Type.Optional(GameFilesSchema),
    saves: Type.Optional(ManifestSavesSchema),
    multiplayer: Type.Optional(MultiplayerSchema),
    online: Type.Optional(
      Type.Object({ join_uri: Type.Optional(Nullable(Str())) }, { additionalProperties: false }),
    ),
    redistribution: Type.Optional(Type.Boolean()),
    maintainers_verified: Type.Optional(Type.Boolean()),
    links: Type.Optional(Type.Record(Type.String(), Nullable(Str()))),
    changelog: Type.Optional(Type.Array(Str())),
  },
  {
    $id: "https://portside.gg/schemas/manifest.v1.json",
    title: "Portside port manifest (schema 1)",
    additionalProperties: false,
  },
);

export type Manifest = Static<typeof ManifestSchema>;

/**
 * File extensions of ROMs, disc images and BIOS dumps. A manifest URL ending in one of these
 * is rejected (docs/legal/policy.md, rule 1).
 */
const ASSET_EXTENSIONS = [
  "z64",
  "n64",
  "v64",
  "sfc",
  "smc",
  "nes",
  "gb",
  "gbc",
  "gba",
  "nds",
  "3ds",
  "cia",
  "iso",
  "cue",
  "bin",
  "chd",
  "cso",
  "gcm",
  "rvz",
  "wbfs",
  "xci",
  "nsp",
  "pbp",
];
// [TBD: host denylist for asset/ROM sites, maintained with docs/legal/policy.md]
const DENYLISTED_HOSTS: string[] = [];

export interface ManifestIssue {
  path: string;
  message: string;
}

export type ManifestResult =
  | { ok: true; manifest: Manifest; warnings: ManifestIssue[] }
  | { ok: false; errors: ManifestIssue[] };

const compiled = TypeCompiler.Compile(ManifestSchema);

function* strings(value: unknown, path: string): Generator<[string, string]> {
  if (typeof value === "string") yield [path, value];
  else if (Array.isArray(value)) {
    for (const [i, v] of value.entries()) yield* strings(v, `${path}/${i}`);
  } else if (value && typeof value === "object") {
    for (const [k, v] of Object.entries(value)) yield* strings(v, `${path}/${k}`);
  }
}

function urlIssue(text: string): string | null {
  for (const match of text.matchAll(/\b[a-z][a-z0-9+.-]*:\/\/[^\s"'<>]+/gi)) {
    let url: URL;
    try {
      url = new URL(match[0]);
    } catch {
      continue;
    }
    const ext = url.pathname.toLowerCase().split(".").pop() ?? "";
    if (url.pathname.includes(".") && ASSET_EXTENSIONS.includes(ext)) {
      return `links to a .${ext} file; manifests must not link to game files`;
    }
    if (DENYLISTED_HOSTS.includes(url.hostname)) return "links to a denylisted host";
  }
  return null;
}

/**
 * Download hosts a `url_list` manifest may use (ADR-0024). Each was looked at by a person:
 * it is the port's own site or CDN, serves https, and keeps a version's files at fixed,
 * versioned URLs, so the sha256 in the manifest stays valid. Adding one is a reviewed change
 * here.
 */
export const URL_LIST_HOSTS: string[] = ["cdn.openttd.org"];

const URL_LIST_SOURCE_FIELDS = ["version", "released", "release_url", "hosts"] as const;
const URL_LIST_BUILD_FIELDS = ["url", "sha256", "size"] as const;
const GITHUB_SOURCE_FIELDS = ["repo", "channel", "tag_pattern", "min_version"] as const;

function urlListIssues(manifest: Manifest, reviewed: string[]): ManifestIssue[] {
  const issues: ManifestIssue[] = [];
  const src = manifest.source;
  if (src.type !== "url_list") {
    for (const f of URL_LIST_SOURCE_FIELDS) {
      if (src[f] !== undefined) issues.push({ path: `/source/${f}`, message: "only for url_list" });
    }
    manifest.builds.forEach((b, i) => {
      for (const f of URL_LIST_BUILD_FIELDS) {
        if (b[f] !== undefined) {
          issues.push({ path: `/builds/${i}/${f}`, message: "only for url_list sources" });
        }
      }
    });
    return issues;
  }
  for (const f of GITHUB_SOURCE_FIELDS) {
    if (src[f] !== undefined) issues.push({ path: `/source/${f}`, message: "not for url_list" });
  }
  for (const f of URL_LIST_SOURCE_FIELDS) {
    if (src[f] === undefined)
      issues.push({ path: `/source/${f}`, message: "required for url_list" });
  }
  if (src.released && Number.isNaN(Date.parse(src.released))) {
    issues.push({ path: "/source/released", message: "not a valid date" });
  }
  const hosts = src.hosts ?? [];
  hosts.forEach((h, i) => {
    if (!reviewed.includes(h)) {
      issues.push({
        path: `/source/hosts/${i}`,
        message: `${h} is not a reviewed download host (URL_LIST_HOSTS)`,
      });
    }
  });
  manifest.builds.forEach((b, i) => {
    for (const f of URL_LIST_BUILD_FIELDS) {
      if (b[f] === undefined) {
        issues.push({ path: `/builds/${i}/${f}`, message: "required for url_list" });
      }
    }
    if (!b.url) return;
    let url: URL;
    try {
      url = new URL(b.url);
    } catch {
      issues.push({ path: `/builds/${i}/url`, message: "not a valid URL" });
      return;
    }
    if (url.protocol !== "https:" || url.username || url.password || url.port) {
      issues.push({ path: `/builds/${i}/url`, message: "must be a plain https URL" });
    }
    if (!hosts.includes(url.hostname)) {
      issues.push({
        path: `/builds/${i}/url`,
        message: `host ${url.hostname} is not in source.hosts`,
      });
    }
    const name = decodeURIComponent(url.pathname.split("/").pop() ?? "");
    if (name !== b.asset) {
      issues.push({ path: `/builds/${i}/asset`, message: `must be the url's file name (${name})` });
    }
  });
  return issues;
}

function gameFilesIssues(manifest: Manifest): ManifestIssue[] {
  const g = manifest.game_files;
  if (!g) return [];
  const issues: ManifestIssue[] = [];
  g.files.forEach((f, i) => {
    const parts = f.replace(/\/$/, "").split("/");
    if (
      f.startsWith("/") ||
      /[\\*]/.test(f) ||
      /^[A-Za-z]:/.test(f) ||
      parts.some((p) => p === "" || p === "." || p === "..")
    ) {
      issues.push({
        path: `/game_files/files/${i}`,
        message: "must be a relative path inside the game's folder",
      });
    }
  });
  for (const [os, dest] of Object.entries(g.dest ?? {})) {
    if (!dest.startsWith("{") || dest.split(/[\\/]/).includes("..")) {
      issues.push({
        path: `/game_files/dest/${os}`,
        message: "must start with a placeholder and stay inside it",
      });
    }
  }
  for (const [store, ids] of Object.entries(g.stores ?? {})) {
    if (new Set(ids.map(String)).size !== ids.length) {
      issues.push({ path: `/game_files/stores/${store}`, message: "lists an id twice" });
    }
  }
  return issues;
}

/** Archive kinds that hold files, so a `nested` archive can be inside them. */
const CONTAINERS: string[] = ["zip", "tar.gz", "tar.xz", "7z"];

/**
 * Validate a parsed manifest against schema 1 plus the rules in manifest-format.md that a
 * schema cannot express. Checks that need the network (asset globs vs. the latest release,
 * upstream id existence) run when the catalog is built (src/releases.ts).
 */
export function validateManifest(
  input: unknown,
  opts: { urlListHosts?: string[] } = {},
): ManifestResult {
  const errors: ManifestIssue[] = [];
  if (!compiled.Check(input)) {
    for (const e of compiled.Errors(input))
      errors.push({ path: e.path || "/", message: e.message });
    return { ok: false, errors };
  }
  const manifest = input;
  for (const [path, text] of strings(manifest, "")) {
    const issue = urlIssue(text);
    if (issue) errors.push({ path, message: issue });
  }
  manifest.builds.forEach((b, i) => {
    if (!b.launch?.exe && !b.launch?.app_bundle && b.archive !== "apk") {
      errors.push({
        path: `/builds/${i}/launch`,
        message: "needs launch.exe or launch.app_bundle",
      });
    }
    if (b.nested && !CONTAINERS.includes(b.archive)) {
      errors.push({
        path: `/builds/${i}/archive`,
        message: `nested needs an outer archive of ${CONTAINERS.join(", ")}`,
      });
    }
    if (b.nested && b.nested.archive === "apk") {
      errors.push({ path: `/builds/${i}/nested/archive`, message: "apk can't be nested" });
    }
    if (b.os === "android" && b.archive === "apk" && !b.android_package) {
      errors.push({ path: `/builds/${i}/android_package`, message: "required for apk builds" });
    }
  });
  const seen = new Set<string>();
  manifest.builds.forEach((b, i) => {
    const key = `${b.os}/${b.arch}`;
    if (seen.has(key)) errors.push({ path: `/builds/${i}`, message: `duplicate build ${key}` });
    seen.add(key);
  });
  if (manifest.source.type.startsWith("github_") && !manifest.source.repo) {
    errors.push({ path: "/source/repo", message: "required for github_* sources" });
  }
  if (manifest.source.channel === "tag_pattern" && !manifest.source.tag_pattern) {
    errors.push({ path: "/source/tag_pattern", message: "required when channel is tag_pattern" });
  }
  errors.push(...urlListIssues(manifest, opts.urlListHosts ?? URL_LIST_HOSTS));
  errors.push(...gameFilesIssues(manifest));
  for (const [i, a] of (manifest.assets_required ?? []).entries()) {
    a.accept.forEach((acc, j) => {
      if (!acc.sha1 && !acc.sha256) {
        errors.push({
          path: `/assets_required/${i}/accept/${j}`,
          message: "needs sha1 or sha256",
        });
      }
    });
  }
  if (errors.length > 0) return { ok: false, errors };
  const warnings: ManifestIssue[] = [];
  if (manifest.redistribution) {
    warnings.push({
      path: "/redistribution",
      message: "redistribution needs written maintainer permission (docs/legal/policy.md)",
    });
  }
  return { ok: true, manifest, warnings };
}
