// Mock answers for Add to Steam (src-tauri/src/steam.rs). `?mock-steam=running` starts with
// Steam open, `?mock-steam=launched` as if Steam started Portside, `?mock-steam=none` without
// Steam. `?mock-play=<port>` starts as if Steam asked for that game (`--play`).
import type { CommandError, SteamStatus } from "../ipc";

const params = new URLSearchParams(location.search);
const mode = params.get("mock-steam");
let playRequest = params.get("mock-play");
const steamRunning = mode === "running" || mode === "launched";
const added = new Set<string>();

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export function steamCommand(cmd: string, a: Record<string, unknown>): unknown {
  switch (cmd) {
    case "steam_status":
      return {
        found: mode !== "none",
        added: added.has(String(a.portId)),
        running: steamRunning,
        started_by_steam: mode === "launched",
      } satisfies SteamStatus;
    case "steam_add":
      return (async () => {
        if (mode === "none") {
          throw {
            code: "E_STEAM_NOT_FOUND",
            message: "Steam isn't installed, or hasn't been signed in to on this computer",
          } satisfies CommandError;
        }
        if (steamRunning && (a.restartSteam !== true || mode === "launched")) {
          throw {
            code: "E_STEAM_RUNNING",
            message: "Steam is open. Close Steam to add games to it.",
          } satisfies CommandError;
        }
        await wait(steamRunning ? 1500 : 400);
        added.add(String(a.portId));
        return null;
      })();
    case "steam_keyboard":
      // No Steam keyboard in a browser; say Steam is closed with `?mock-steam=none`.
      console.info("[mock] Steam's keyboard opened");
      return mode !== "none";
    case "play_request": {
      const id = playRequest;
      playRequest = null;
      return id;
    }
    default:
      return undefined;
  }
}
