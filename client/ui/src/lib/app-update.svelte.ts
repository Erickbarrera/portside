// Updates to Portside itself: what the last check found, and installing it. Shared by the
// banner in the layout and Settings.
import { events, type AppUpdateStatus, type CommandError, commands } from "./ipc";

let status = $state<AppUpdateStatus | null>(null);
let installError = $state<CommandError | null>(null);
// The banner was closed for this version; Settings still offers it.
let dismissed = $state<string | null>(null);
let started = false;

export const appUpdate = {
  get status() {
    return status;
  },
  /** Why the last Update and restart didn't go ahead (a game running, a failed download). */
  get installError() {
    return installError;
  },
  /** True while the banner should offer the update. */
  get showBanner() {
    const version = status?.available?.version;
    return !!version && version !== dismissed;
  },
  /** Loads the status and follows changes. Safe to call more than once. */
  async start() {
    if (started) return;
    started = true;
    status = await commands.appUpdateStatus();
    await events.appUpdateChanged.listen(({ payload }) => {
      status = payload.status;
    });
  },
  async check() {
    installError = null;
    status = await commands.appUpdateCheck();
  },
  /** Downloads and installs the update; Portside restarts when it's done. */
  async install() {
    installError = null;
    const result = await commands.appUpdateInstall();
    if (result.status === "error") installError = result.error;
  },
  dismiss() {
    dismissed = status?.available?.version ?? null;
  },
};

/** "45%" or "12.3 MB", for a progress line. */
export function progressLabel(downloaded: number, total: number | null): string {
  if (total) return `${Math.min(100, Math.floor((downloaded / total) * 100))}%`;
  return `${(downloaded / 1_000_000).toFixed(1)} MB`;
}
