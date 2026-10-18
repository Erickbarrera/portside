// Mock of Portside's own updates. `?mock-update` starts with a newer version found;
// installing it pretends to download, then reloads the page as the restart.
import { emit } from "@tauri-apps/api/event";
import type { AppUpdateStatus } from "../ipc";

const params = new URLSearchParams(location.search);

let status: AppUpdateStatus = {
  off_reason: null,
  checking: false,
  last_checked_at: Date.now() / 1000 - 300,
  available: params.has("mock-update")
    ? {
        version: "0.2.0-mock",
        notes: "## What's new\n- Example improvement to the Library.\n- Example fix for installs.",
      }
    : null,
  progress: null,
  last_error: null,
};

function set(next: Partial<AppUpdateStatus>) {
  status = { ...status, ...next };
  void emit("app-update-changed", { status });
}

export function appUpdateCommand(cmd: string): unknown {
  switch (cmd) {
    case "app_update_status":
      return status;
    case "app_update_check":
      set({ last_checked_at: Date.now() / 1000 });
      return status;
    case "app_update_install": {
      const total = 18_000_000;
      let downloaded = 0;
      set({ progress: { downloaded, total } });
      const timer = setInterval(() => {
        downloaded = Math.min(total, downloaded + 1_500_000);
        set({ progress: { downloaded, total } });
        if (downloaded === total) {
          clearInterval(timer);
          setTimeout(() => location.reload(), 500);
        }
      }, 150);
      return null;
    }
    default:
      return undefined;
  }
}
