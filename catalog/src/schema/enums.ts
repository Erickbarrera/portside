import { type TLiteral, type TUnion, Type } from "@sinclair/typebox";

/** Union of string literals that keeps the literal types (`Static` = "a" | "b"). */
const literals = <const T extends readonly string[]>(values: T, description?: string) =>
  Type.Union(
    values.map((v) => Type.Literal(v)),
    description ? { description } : {},
  ) as unknown as TUnion<{ -readonly [K in keyof T]: TLiteral<T[K]> }>;

export const OS_VALUES = ["windows", "macos", "linux", "android"] as const;
export const ARCH_VALUES = ["x86_64", "arm64", "universal"] as const;
export const STATUS_VALUES = [
  "perfect",
  "playable",
  "in_progress",
  "boots",
  "not_playable",
] as const;
export const PORT_TYPE_VALUES = [
  "static-recomp",
  "source-port",
  "engine-remake",
  "browser-port",
  "decompilation",
  "disassembly",
  "tool",
] as const;
export const ARCHIVE_VALUES = [
  "zip",
  "tar.gz",
  "tar.xz",
  "7z",
  "none",
  "apk",
  "dmg",
  "appimage",
] as const;
export const MULTIPLAYER_VALUES = ["none", "local", "online_coop", "online_pvp"] as const;
export const STATUS_SOURCE_VALUES = ["curator", "manifest", "seed"] as const;

export type Os = (typeof OS_VALUES)[number];
export type Arch = (typeof ARCH_VALUES)[number];
export type PlayableStatus = (typeof STATUS_VALUES)[number];
export type PortType = (typeof PORT_TYPE_VALUES)[number];
export type StatusSource = (typeof STATUS_SOURCE_VALUES)[number];

export const OsSchema = literals(OS_VALUES);
export const ArchSchema = literals(ARCH_VALUES);
export const PlayableStatusSchema = literals(STATUS_VALUES, "docs/specs/catalog.md");
export const PortTypeSchema = literals(PORT_TYPE_VALUES, "Upstream port type");
export const ArchiveSchema = literals(ARCHIVE_VALUES);
export const MultiplayerSchema = literals(MULTIPLAYER_VALUES);
export const StatusSourceSchema = literals(STATUS_SOURCE_VALUES);
