// Cloud saves in the browser-only mock. `?mock-saves=on` starts set up; `?mock-saves=conflict`
// also gives every game a conflict to settle. Paths and devices are synthetic.
import { emit } from "@tauri-apps/api/event";
import type {
  CommandError,
  DeviceRecord,
  GameSaves,
  SavesChanged,
  SavesStatus,
  SnapshotInfo,
} from "../ipc";

const mode = new URLSearchParams(location.search).get("mock-saves");
let location_: string | null = mode ? "/home/example/Dropbox" : null;
const conflicts = new Set<string>();
const synced = new Map<string, number>();
const custom = new Map<string, string>();

const devices: DeviceRecord[] = [
  {
    id: "dev-1",
    name: "Example Desktop",
    os: "linux",
    app_version: "0.1.0",
    last_sync: new Date().toISOString(),
  },
  {
    id: "dev-2",
    name: "Example Laptop",
    os: "macos",
    app_version: "0.1.0",
    last_sync: new Date(Date.now() - 86_400_000).toISOString(),
  },
];

function status(): SavesStatus {
  return {
    enabled: location_ !== null,
    location: location_,
    problem: null,
    device_name: "Example Desktop",
    suggestions: [
      { label: "Dropbox", path: "/home/example/Dropbox", has_saves: true },
      { label: "Google Drive", path: "/home/example/Google Drive", has_saves: false },
    ],
  };
}

function side(id: string, thisDevice: boolean, hoursAgo: number, files: number): SnapshotInfo {
  return {
    id,
    device_name: thisDevice ? "Example Desktop" : "Example Laptop",
    os: thisDevice ? "linux" : "macos",
    this_device: thisDevice,
    created_at: new Date(Date.now() - hoursAgo * 3_600_000).toISOString(),
    file_count: files,
    size: files * 48_000,
    port_version: "v1.2.0",
  };
}

function game(portId: string, syncing = false): GameSaves {
  const folder = custom.get(portId) ?? `/home/example/.local/share/${portId}/saves`;
  const base: GameSaves = {
    port_id: portId,
    status: "not_synced",
    folder,
    custom_folder: custom.has(portId),
    last_sync: synced.get(portId) ?? null,
    last_result: null,
    message: null,
    current: null,
    conflict: [],
  };
  if (!location_) return { ...base, status: "off", folder: null };
  if (syncing) return { ...base, status: "syncing" };
  if (conflicts.has(portId)) {
    return {
      ...base,
      status: "conflict",
      last_result: "conflict",
      conflict: [side("snap-laptop", false, 2, 3), side("snap-here", true, 26, 2)],
    };
  }
  return synced.has(portId) ? { ...base, status: "synced", last_result: "up_to_date" } : base;
}

function changed(saves: GameSaves) {
  void emit("saves-changed", { saves } satisfies SavesChanged);
}

/** A launch that syncs first; throws like Rust does when a conflict blocks it. */
export function mockBeforeLaunch(portId: string, skip: boolean): void {
  if (!location_ || skip) return;
  if (mode === "conflict" && !synced.has(portId)) conflicts.add(portId);
  if (conflicts.has(portId)) {
    changed(game(portId));
    throw {
      code: "E_SAVES_CONFLICT",
      message: "This game has different saves on another device. Choose which to keep.",
    } satisfies CommandError;
  }
  synced.set(portId, Date.now() / 1000);
  changed(game(portId));
}

export function mockAfterExit(portId: string): void {
  if (!location_ || conflicts.has(portId)) return;
  changed(game(portId, true));
  setTimeout(() => {
    synced.set(portId, Date.now() / 1000);
    changed(game(portId));
  }, 800);
}

export function savesCommand(cmd: string, a: Record<string, unknown>): unknown {
  const portId = String(a.portId ?? "");
  switch (cmd) {
    case "saves_status":
      return status();
    case "saves_pick_folder":
      return "/home/example/Sync";
    case "saves_use_folder":
      location_ = String(a.path);
      return status();
    case "saves_turn_off":
      location_ = null;
      return status();
    case "saves_devices":
      return devices;
    case "saves_game":
      if (mode === "conflict" && location_ && !synced.has(portId)) conflicts.add(portId);
      return game(portId);
    case "saves_sync":
      if (!conflicts.has(portId)) synced.set(portId, Date.now() / 1000);
      return game(portId);
    case "saves_keep":
      conflicts.delete(portId);
      synced.set(portId, Date.now() / 1000);
      return { ...game(portId), last_result: "restored" } satisfies GameSaves;
    case "saves_set_game_folder":
      if (a.path) custom.set(portId, String(a.path));
      else custom.delete(portId);
      return game(portId);
    case "saves_reveal":
      return null;
    default:
      return undefined;
  }
}
