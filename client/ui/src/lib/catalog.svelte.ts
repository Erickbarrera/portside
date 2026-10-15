// Freshness of the catalog saved on this device, shared by Discover and Settings.
import { events, type CatalogStatus, commands } from "./ipc";

let status = $state<CatalogStatus | null>(null);
let refreshing = $state(false);
let started = false;
const listeners = new Set<() => void>();

export const catalog = {
  get status() {
    return status;
  },
  get refreshing() {
    return refreshing;
  },
  /** Loads the status and follows sync events. Safe to call more than once. */
  async start() {
    if (started) return;
    started = true;
    const result = await commands.catalogStatus();
    if (result.status === "ok") status = result.data;
    await events.catalogSynced.listen(({ payload }) => {
      status = payload.status;
      if (payload.changed) for (const fn of listeners) fn();
    });
  },
  /** Asks the server for changes now. */
  async refresh() {
    refreshing = true;
    const result = await commands.catalogRefresh();
    refreshing = false;
    if (result.status === "ok") status = result.data;
  },
  /** Runs `fn` whenever a sync changes the saved catalog. Returns an unsubscribe. */
  onChange(fn: () => void) {
    listeners.add(fn);
    return () => listeners.delete(fn);
  },
};
