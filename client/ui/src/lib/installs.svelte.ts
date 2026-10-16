// Installed games and running installs, shared by the game page and the Library.
import {
  events,
  type CommandError,
  type InstallState,
  type Installed,
  type PlanSummary,
  type Storage,
  type StorageKind,
  commands,
} from "./ipc";

const installed = $state<Record<string, Installed>>({});
const active = $state<Record<string, InstallState>>({});
/** The last failure per port, until the next attempt. */
const failed = $state<Record<string, CommandError>>({});
let loaded = $state(false);
/** Where games can go: internal storage, plus a Steam Deck's SD card while it's in. */
let storage = $state<Storage[]>([]);
let started: Promise<void> | null = null;

function apply(portId: string, state: InstallState) {
  switch (state.state) {
    case "installed":
      installed[portId] = state.install;
      delete active[portId];
      break;
    case "uninstalled":
      delete installed[portId];
      delete active[portId];
      break;
    case "failed":
      failed[portId] = state.error;
      delete active[portId];
      break;
    case "cancelled":
      delete active[portId];
      break;
    default:
      active[portId] = state;
  }
}

export const installs = {
  get installed() {
    return installed;
  },
  get active() {
    return active;
  },
  get failed() {
    return failed;
  },
  get loaded() {
    return loaded;
  },
  get storage() {
    return storage;
  },
  /** Checks again for an SD card (it can be put in or taken out at any time). */
  async refreshStorage() {
    const result = await commands.installStorage();
    if (result.status === "ok") storage = result.data;
  },
  /** Loads the library and follows install events. Safe to call more than once. */
  start(): Promise<void> {
    started ??= (async () => {
      await events.installChanged.listen(({ payload }) => apply(payload.port_id, payload.state));
      const [list, running] = await Promise.all([
        commands.libraryList(),
        commands.installsActive(),
      ]);
      if (list.status === "ok") {
        for (const i of list.data) installed[i.port_id] = i;
      }
      for (const r of running) apply(r.port_id, r.state);
      loaded = true;
      const places = await commands.installStorage();
      if (places.status === "ok") storage = places.data;
    })();
    return started;
  },
  plan(portId: string) {
    return commands.installPlan(portId);
  },
  async install(plan: PlanSummary, acceptUnsigned: boolean, to: StorageKind | null = null) {
    delete failed[plan.port_id];
    active[plan.port_id] = { state: "downloading", done: 0, total: plan.size };
    const result = await commands.installStart(plan.port_id, acceptUnsigned, to);
    if (result.status === "error") {
      delete active[plan.port_id];
      failed[plan.port_id] = result.error;
    }
    return result;
  },
  cancel(portId: string) {
    return commands.installCancel(portId);
  },
  async uninstall(portId: string) {
    const result = await commands.uninstall(portId);
    if (result.status === "ok") delete installed[portId];
    return result;
  },
  reveal(portId: string) {
    return commands.installReveal(portId);
  },
};

/** The storage an installed game is on, by its folder. */
export function storageOf(install: Installed): Storage | undefined {
  return storage.find(
    (s) => install.dir.startsWith(`${s.path}/`) || install.dir.startsWith(`${s.path}\\`),
  );
}

/** 0–1 download progress, or null when the size is unknown. */
export function fraction(state: InstallState): number | null {
  if (state.state !== "downloading") return null;
  return state.total ? Math.min(1, state.done / state.total) : null;
}
