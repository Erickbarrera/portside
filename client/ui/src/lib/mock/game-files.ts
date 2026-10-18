// Mock answers for game files from the player's own copy (src-tauri/src/game_files.rs).
// By default Steam has the game, so installing copies the files. `?mock-game-files=none`
// finds no copy (Choose folder picks one that works), `=both` finds it in Steam and GOG.
import type { CommandError, Found, GameFilesStatus } from "../ipc";
import { portDetail } from "./fixtures";

const mode = new URLSearchParams(location.search).get("mock-game-files");
const copiedFrom = new Map<string, Found>();

function found(): Found[] {
  const steam: Found = {
    store: "steam",
    dir: "/home/example/.local/share/Steam/steamapps/common/Example RPG",
  };
  const gog: Found = { store: "gog", dir: "/home/example/Games/Heroic/Example RPG" };
  if (mode === "none") return [];
  return mode === "both" ? [steam, gog] : [steam];
}

function status(portId: string, installed: boolean): GameFilesStatus | null {
  const spec = portDetail(portId)?.manifest?.game_files;
  if (!spec) return null;
  return {
    label: spec.label,
    files: spec.files,
    ready: installed && copiedFrom.has(portId),
    source: copiedFrom.get(portId) ?? null,
    found: found(),
  };
}

/** After a (fake) install: copies from the copy used before, else the first one found. */
export function mockGameFilesAfterInstall(portId: string): boolean {
  if (!portDetail(portId)?.manifest?.game_files) return false;
  const from = copiedFrom.get(portId) ?? found()[0];
  if (from) copiedFrom.set(portId, from);
  return true;
}

export function gameFilesCommand(
  cmd: string,
  a: Record<string, unknown>,
  installed: (portId: string) => boolean,
): unknown {
  const portId = String(a.portId);
  switch (cmd) {
    case "game_files_status":
      return status(portId, installed(portId));
    case "game_files_copy":
      return (async () => {
        await new Promise((resolve) => setTimeout(resolve, 800));
        if (!installed(portId)) {
          throw {
            code: "E_NOT_INSTALLED",
            message: `${portId} is not installed.`,
          } satisfies CommandError;
        }
        copiedFrom.set(portId, { store: a.store as Found["store"], dir: String(a.dir) });
        return status(portId, true);
      })();
    default:
      return undefined;
  }
}
