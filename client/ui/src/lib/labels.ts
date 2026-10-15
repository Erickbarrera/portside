import type { Arch, Os, PlayableStatus, PortType } from "./ipc";

// Status names from docs/specs/catalog.md. Status is never shown by color alone.
export const statusLabel: Record<PlayableStatus, string> = {
  perfect: "Perfect",
  playable: "Playable",
  in_progress: "In progress",
  boots: "Boots",
  not_playable: "Not playable",
};

export const statusOrder: PlayableStatus[] = [
  "perfect",
  "playable",
  "in_progress",
  "boots",
  "not_playable",
];

export const osLabel: Record<Os, string> = {
  windows: "Windows",
  macos: "macOS",
  linux: "Linux",
  android: "Android",
};

export const archLabel: Record<Arch, string> = {
  x86_64: "x86-64",
  arm64: "ARM64",
  universal: "Universal",
};

export const typeLabel: Record<PortType, string> = {
  "static-recomp": "Static recomp",
  "source-port": "Source port",
  "engine-remake": "Engine remake",
  "browser-port": "Browser port",
  decompilation: "Decompilation",
  disassembly: "Disassembly",
  tool: "Tool",
};

const numberFormat = new Intl.NumberFormat("en");
export const formatNumber = (n: number) => numberFormat.format(n);

const dateFormat = new Intl.DateTimeFormat("en", { dateStyle: "medium" });
export function formatDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : dateFormat.format(d);
}

export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value < 10 && unit > 0 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

/** "just now", "5 minutes ago", "3 hours ago", else a date. `unixSeconds` from Rust. */
export function formatAgo(unixSeconds: number, now = Date.now()): string {
  const minutes = Math.round((now / 1000 - unixSeconds) / 60);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} minute${minutes === 1 ? "" : "s"} ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} hour${hours === 1 ? "" : "s"} ago`;
  return dateFormat.format(new Date(unixSeconds * 1000));
}
