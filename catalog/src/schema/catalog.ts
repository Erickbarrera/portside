/**
 * Shapes of the static catalog files. Contract: docs/specs/static-catalog.md.
 * `pnpm contract:write` turns these into docs/specs/static-catalog/v1.json, which the desktop
 * app's contract test reads.
 */
import { type Static, type TSchema, Type } from "@sinclair/typebox";
import {
  ArchSchema,
  OsSchema,
  PlayableStatusSchema,
  PortTypeSchema,
  StatusSourceSchema,
} from "./enums.js";
import { ManifestSchema } from "./manifest.js";

const Nullable = <T extends TSchema>(t: T) => Type.Union([t, Type.Null()]);
const DateTime = Type.String({ format: "date-time" });

export const OsStatusSchema = Type.Object({
  os: OsSchema,
  status: PlayableStatusSchema,
  source: StatusSourceSchema,
});

export const RatingsSchema = Type.Object({
  playability: Type.Optional(Type.Integer({ minimum: 1, maximum: 5 })),
  completeness: Type.Optional(Type.Integer({ minimum: 1, maximum: 5 })),
  activity: Type.Optional(Type.Integer({ minimum: 1, maximum: 5 })),
  popularity: Type.Optional(Type.Integer({ minimum: 1, maximum: 5 })),
});

export const ReleaseAssetSchema = Type.Object({
  os: OsSchema,
  arch: ArchSchema,
  name: Type.String(),
  url: Type.String(),
  size: Type.Integer({ minimum: 0 }),
  sha256: Nullable(Type.String()),
});

export const ReleaseSchema = Type.Object({
  id: Type.String(),
  port_id: Type.String(),
  tag: Type.String(),
  name: Nullable(Type.String()),
  published_at: DateTime,
  prerelease: Type.Boolean(),
  revoked: Type.Boolean(),
  html_url: Type.String(),
  assets: Type.Array(ReleaseAssetSchema),
});

export const PortSummarySchema = Type.Object(
  {
    id: Type.String(),
    game_key: Type.String(),
    title: Type.String(),
    type: PortTypeSchema,
    status: PlayableStatusSchema,
    os_status: Type.Array(OsStatusSchema),
    stars: Nullable(Type.Integer()),
    archived: Type.Boolean(),
    screenshot_url: Nullable(Type.String()),
    play_url: Nullable(Type.String()),
    updated_at: DateTime,
  },
  { $id: "PortSummary" },
);

export const PortDetailSchema = Type.Composite(
  [
    Type.Omit(PortSummarySchema, []),
    Type.Object({
      description: Nullable(Type.String()),
      repo: Nullable(Type.String()),
      homepage: Nullable(Type.String()),
      license: Nullable(Type.String()),
      last_commit: Nullable(DateTime),
      upstream_score: Nullable(Type.Number()),
      ratings: Nullable(RatingsSchema),
      video_url: Nullable(Type.String()),
      note: Nullable(Type.String()),
      status_notes: Nullable(Type.String()),
      manifest: Nullable(ManifestSchema),
      manifest_rev: Nullable(Type.String()),
      verified: Type.Boolean(),
      latest_releases: Type.Array(ReleaseSchema),
    }),
  ],
  { $id: "PortDetail" },
);

export const GameSchema = Type.Object({
  game_key: Type.String(),
  title: Type.String(),
  original_platforms: Type.Array(Type.String()),
  ports: Type.Array(PortSummarySchema),
});

export const CatalogSnapshotSchema = Type.Object(
  {
    rev: Type.Integer({
      minimum: 0,
      description: "Unix seconds of the build that last changed the catalog.",
    }),
    ports: Type.Array(PortSummarySchema),
  },
  { $id: "CatalogSnapshot" },
);

export type OsStatus = Static<typeof OsStatusSchema>;
export type Ratings = Static<typeof RatingsSchema>;
export type Release = Static<typeof ReleaseSchema>;
export type ReleaseAsset = Static<typeof ReleaseAssetSchema>;
export type PortSummary = Static<typeof PortSummarySchema>;
export type PortDetail = Static<typeof PortDetailSchema>;
export type Game = Static<typeof GameSchema>;
export type CatalogSnapshot = Static<typeof CatalogSnapshotSchema>;
