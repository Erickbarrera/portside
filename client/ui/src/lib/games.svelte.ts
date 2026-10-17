// Running games and play time, shared by the game page and the Library.
import { events, type CommandError, type GameState, type PlayStats, commands } from "./ipc";

type Live = Extract<GameState, { state: "running" | "stopping" }>;
type Ended = Extract<GameState, { state: "exited" }>;

const running = $state<Record<string, Live>>({});
const stats = $state<Record<string, PlayStats>>({});
/** How each game's last run from this session ended. */
const ended = $state<Record<string, Ended>>({});
/** Why the last Play didn't start, per game. */
const failed = $state<Record<string, CommandError>>({});
let started: Promise<void> | null = null;

function apply(portId: string, state: GameState) {
  if (state.state === "exited") {
    delete running[portId];
    stats[portId] = state.stats;
    ended[portId] = state;
  } else {
    running[portId] = state;
    delete ended[portId];
  }
}

export const games = {
  get running() {
    return running;
  },
  get stats() {
    return stats;
  },
  get ended() {
    return ended;
  },
  get failed() {
    return failed;
  },
  /** Loads play time and follows game events. Safe to call more than once. */
  start(): Promise<void> {
    started ??= (async () => {
      await events.gameChanged.listen(({ payload }) => apply(payload.port_id, payload.state));
      // Play time from the player's other devices, through cloud saves.
      await events.playtimeChanged.listen(({ payload }) => {
        for (const s of payload.stats) stats[s.port_id] = s;
      });
      const [list, live] = await Promise.all([commands.playStats(), commands.gamesRunning()]);
      if (list.status === "ok") {
        for (const s of list.data) stats[s.port_id] = s;
      }
      for (const g of live) apply(g.port_id, g.state);
    })();
    return started;
  },
  /** `skipSaveSync` starts the game without syncing cloud saves first ("Play anyway"). */
  async launch(portId: string, skipSaveSync = false) {
    delete failed[portId];
    delete ended[portId];
    const result = await commands.gameLaunch(portId, skipSaveSync);
    if (result.status === "error") failed[portId] = result.error;
    return result;
  },
  stop(portId: string) {
    return commands.gameStop(portId);
  },
  revealLog(portId: string) {
    return commands.gameRevealLog(portId);
  },
};

/** A run that ended on its own with a failure code, worth pointing at the log. */
export function crashed(state: Ended | undefined): boolean {
  return !!state && !state.stopped && state.code !== null && state.code !== 0;
}

/** Library order: recently played first (playing now on top, never played last, by title). */
export function byRecent(
  a: { port_id: string; title: string },
  b: { port_id: string; title: string },
  last: (portId: string) => number | null,
): number {
  const la = last(a.port_id) ?? -1;
  const lb = last(b.port_id) ?? -1;
  return lb - la || a.title.localeCompare(b.title);
}

/** "12 minutes", "3.5 hours"; null when never played. */
export function formatPlaytime(secs: number | undefined): string | null {
  if (!secs) return null;
  const minutes = Math.round(secs / 60);
  if (minutes < 1) return "Less than a minute";
  if (minutes < 60) return minutes === 1 ? "1 minute" : `${minutes} minutes`;
  const hours = secs / 3600;
  const shown = hours < 10 ? Math.round(hours * 10) / 10 : Math.round(hours);
  return shown === 1 ? "1 hour" : `${shown} hours`;
}
