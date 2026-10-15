// Installed games and running installs, shared by the game page and the Library.
import {
  events,
  type CommandError,
  type InstallState,
  type Installed,
  type PlanSummary,
  commands,
} from "./ipc";

const installed = $state<Record<string, Installed>>({});
const active = $state<Record<string, InstallState>>({});
/** The last failure per port, until the next attempt. */
const failed = $state<Record<string, CommandError>>({});
let loaded = $state(false);
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
    })();
    return started;
  },
  plan(portId: string) {
    return commands.installPlan(portId);
  },
  async install(plan: PlanSummary, acceptUnsigned: boolean) {
    delete failed[plan.port_id];
    active[plan.port_id] = { state: "downloading", done: 0, total: plan.size };
    const result = await commands.installStart(plan.port_id, acceptUnsigned);
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

/** 0–1 download progress, or null when the size is unknown. */
export function fraction(state: InstallState): number | null {
  if (state.state !== "downloading") return null;
  return state.total ? Math.min(1, state.done / state.total) : null;
}
