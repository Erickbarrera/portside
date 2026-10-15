// Cloud saves: this device's setup and each game's sync state (docs/save-sync.md).
import { events, type GameSaves, type SaveStatus, type SavesStatus, commands } from "./ipc";

let status = $state<SavesStatus | null>(null);
const games = $state<Record<string, GameSaves>>({});
let started: Promise<void> | null = null;

export const saves = {
  get status() {
    return status;
  },
  get games() {
    return games;
  },
  /** Follows sync events and loads this device's setup. Safe to call more than once. */
  start(): Promise<void> {
    started ??= (async () => {
      await events.savesChanged.listen(({ payload }) => {
        games[payload.saves.port_id] = payload.saves;
      });
      await saves.refresh();
    })();
    return started;
  },
  async refresh() {
    const result = await commands.savesStatus();
    if (result.status === "ok") status = result.data;
    return result;
  },
  /** Sets the device status from a command that changed it, and forgets per-game state. */
  set(next: SavesStatus) {
    status = next;
    for (const id of Object.keys(games)) delete games[id];
  },
  async load(portId: string) {
    const result = await commands.savesGame(portId);
    if (result.status === "ok") games[portId] = result.data;
    return result;
  },
  async sync(portId: string) {
    const result = await commands.savesSync(portId);
    if (result.status === "ok") games[portId] = result.data;
    return result;
  },
  async keep(portId: string, snapshotId: string) {
    const result = await commands.savesKeep(portId, snapshotId);
    if (result.status === "ok") games[portId] = result.data;
    return result;
  },
  async setFolder(portId: string, path: string | null) {
    const result = await commands.savesSetGameFolder(portId, path);
    if (result.status === "ok") games[portId] = result.data;
    return result;
  },
  reveal(portId: string) {
    return commands.savesReveal(portId);
  },
};

export const saveStatusLabel: Record<SaveStatus, string> = {
  off: "Off",
  no_folder: "Save folder unknown",
  not_synced: "Not synced yet",
  syncing: "Syncing…",
  synced: "Synced",
  conflict: "Choose which saves to keep",
  unavailable: "Can't reach your cloud folder",
  error: "Sync failed",
};

/** "3 files · 1.2 MB". */
export function describeFiles(count: number, size: string): string {
  return `${count} ${count === 1 ? "file" : "files"} · ${size}`;
}
