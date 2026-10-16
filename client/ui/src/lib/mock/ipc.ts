// Browser-only stand-in for the Rust side, so `pnpm --filter @portside/ui dev` works without
// Tauri. Answers the commands in bindings.ts from synthetic fixtures.
import { emit } from "@tauri-apps/api/event";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type {
  AppInfo,
  CatalogQuery,
  CatalogStatus,
  CommandError,
  GameChanged,
  GameState,
  InstallChanged,
  InstallState,
  Installed,
  Os,
  Page,
  PlanSummary,
  PlayStats,
  PortSummary,
  Storage,
  StorageKind,
} from "../ipc";
import { portDetail, ports } from "./fixtures";
import { mockAfterExit, mockBeforeLaunch, savesCommand } from "./saves";
import { socialCommand } from "./social";
import { steamCommand } from "./steam";

// `?mock-os=macos` previews another system (e.g. the unsigned-app warning).
const params = new URLSearchParams(location.search);
const mockOs = (params.get("mock-os") as Os | null) ?? "linux";
// `?mock-crash` makes every game quit with an error a moment after it starts.
const mockCrash = params.has("mock-crash");
// `?mock-sd` is a Steam Deck with an SD card in (installs can go there).
const SD_LIBRARY = "/run/media/deck/Example Card/Portside";
const storage: Storage[] = [
  { kind: "internal", path: "/mock/library", free: 41_000_000_000 },
  ...(params.has("mock-sd")
    ? [{ kind: "sd_card" as const, path: SD_LIBRARY, free: 212_000_000_000 }]
    : []),
];
const installed = new Map<string, Installed>();
const running = new Map<string, { state: InstallState; timer: ReturnType<typeof setInterval> }>();
const playing = new Map<string, number>();
const played = new Map<string, PlayStats>();

const status = (): CatalogStatus => ({
  port_count: ports.length,
  last_synced_at: Date.now() / 1000 - 120,
  last_error: null,
});

const notFound: CommandError = { code: "E_NOT_FOUND", message: "Not found." };

function search(query: CatalogQuery): Page<PortSummary> {
  const q = query.q?.trim().toLowerCase();
  const items = ports.filter(
    (p) =>
      (!q || p.title.toLowerCase().includes(q)) &&
      (!query.os || p.os_status.some((s) => s.os === query.os)) &&
      (!query.status || p.status === query.status) &&
      (!query.port_type || p.type === query.port_type),
  );
  return { items, cursor: null };
}

function plan(id: string): PlanSummary {
  const port = portDetail(id);
  const build = port?.manifest?.builds.find((b) => b.os === mockOs);
  const asset = port?.latest_releases[0]?.assets.find((a) => a.os === mockOs);
  if (!port || !build || !asset) {
    throw { code: "E_NO_BUILD", message: `Not available for ${mockOs}.` } satisfies CommandError;
  }
  return {
    port_id: id,
    title: port.title,
    version: "v1.2.0",
    release_url: port.latest_releases[0].html_url,
    repo: `example/${id}`,
    asset: asset.name,
    size: asset.size,
    os: build.os,
    arch: build.arch,
    fit: build.arch === "universal" ? "universal" : "native",
    unsigned: mockOs === "macos",
    verification: "none",
    installed_version: installed.get(id)?.version ?? null,
  };
}

function send(port_id: string, state: InstallState) {
  void emit("install-changed", { port_id, state } satisfies InstallChanged);
}

/** The library entry a finished (fake) install of `id` leaves behind. */
function installedEntry(
  id: string,
  installedAt = Date.now() / 1000,
  library = "/mock/library",
): Installed {
  const p = plan(id);
  return {
    port_id: id,
    title: p.title,
    version: p.version,
    dir: `${library}/${id}/${p.version}`,
    os: p.os,
    arch: p.arch,
    fit: p.fit,
    asset: p.asset,
    release_url: p.release_url,
    size: p.size,
    sha256: null,
    verified: "none",
    installed_at: installedAt,
    unsigned: p.unsigned,
    build: { os: p.os, arch: p.arch, asset: p.asset, archive: "zip" },
  };
}

// `?mock-installed` starts with every game that has a build for this system installed.
if (params.has("mock-installed")) {
  ports.forEach((port, i) => {
    try {
      installed.set(port.id, installedEntry(port.id, Date.now() / 1000 - (i + 1) * 86400));
    } catch {
      // No build for this system.
    }
  });
}

/** Pretends to download 48 MB in about four seconds. */
function fakeInstall(id: string, to: StorageKind | null) {
  const p = plan(id);
  // Updates stay where the game is.
  const library =
    installed.get(id)?.dir.replace(/\/[^/]+\/[^/]+$/, "") ??
    storage.find((s) => s.kind === (to ?? "internal"))?.path;
  let done = 0;
  const timer = setInterval(() => {
    done = Math.min(p.size, done + p.size / 16);
    let state: InstallState = { state: "downloading", done, total: p.size };
    if (done >= p.size) {
      clearInterval(timer);
      running.delete(id);
      const install = installedEntry(id, undefined, library);
      installed.set(id, install);
      state = { state: "installed", install };
    } else {
      running.set(id, { state, timer });
    }
    send(id, state);
  }, 250);
  running.set(id, { state: { state: "downloading", done: 0, total: p.size }, timer });
}

function sendGame(port_id: string, state: GameState) {
  void emit("game-changed", { port_id, state } satisfies GameChanged);
}

function fakeLaunch(id: string) {
  if (!installed.has(id)) {
    throw { code: "E_NOT_INSTALLED", message: `${id} is not installed.` } satisfies CommandError;
  }
  if (playing.has(id)) {
    throw {
      code: "E_GAME_RUNNING",
      message: "This game is running. Quit it first.",
    } satisfies CommandError;
  }
  const started_at = Math.floor(Date.now() / 1000);
  playing.set(id, started_at);
  sendGame(id, { state: "running", started_at });
  if (mockCrash) setTimeout(() => endGame(id, 1, false), 1500);
}

/** Ends a fake game; each mock session counts as 25 minutes so play time shows. */
function fakeStop(id: string): boolean {
  const started_at = playing.get(id);
  if (started_at === undefined) return false;
  sendGame(id, { state: "stopping", started_at });
  setTimeout(() => endGame(id, null, true), 600);
  return true;
}

function endGame(id: string, code: number | null, stopped: boolean) {
  if (!playing.delete(id)) return;
  const before = played.get(id);
  const stats: PlayStats = {
    port_id: id,
    total_secs: (before?.total_secs ?? 0) + 25 * 60,
    last_played: Math.floor(Date.now() / 1000),
    sessions: (before?.sessions ?? 0) + 1,
  };
  played.set(id, stats);
  sendGame(id, { state: "exited", code, stopped, stats });
  mockAfterExit(id);
}

export function installMockIpc() {
  mockWindows("main");
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      switch (cmd) {
        case "app_info":
          return {
            version: "0.1.0-mock",
            os: mockOs,
            arch: "x86_64",
            catalog_url: "mock://synthetic-fixtures/",
            // `?mock-couch` starts as if on a Steam Deck in Game Mode.
            big_screen: params.has("mock-couch"),
            // `?mock-deck` is a Steam Deck (text fields ask for Steam's keyboard).
            steam_deck: params.has("mock-deck"),
          } satisfies AppInfo;
        case "catalog_search":
          return search(a.query as CatalogQuery);
        case "port_get": {
          const port = portDetail(String(a.id));
          if (!port) throw notFound;
          return { port, offline: false };
        }
        case "catalog_summaries":
          return (a.ids as string[]).flatMap((id) => ports.filter((p) => p.id === id));
        case "catalog_status":
        case "catalog_refresh":
          return status();
        case "install_plan":
          return plan(String(a.portId));
        case "install_start":
          fakeInstall(String(a.portId), (a.to as StorageKind | null) ?? null);
          return null;
        case "install_storage":
          return storage;
        case "install_cancel": {
          const job = running.get(String(a.portId));
          if (!job) return false;
          clearInterval(job.timer);
          running.delete(String(a.portId));
          send(String(a.portId), { state: "cancelled" });
          return true;
        }
        case "installs_active":
          return [...running].map(([port_id, j]) => ({ port_id, state: j.state }));
        case "library_list":
          return [...installed.values()];
        case "library_get":
          return installed.get(String(a.portId)) ?? null;
        case "uninstall":
          installed.delete(String(a.portId));
          send(String(a.portId), { state: "uninstalled" });
          return null;
        case "install_reveal":
        case "game_reveal_log":
          return null;
        case "game_launch":
          mockBeforeLaunch(String(a.portId), a.skipSaveSync === true);
          fakeLaunch(String(a.portId));
          return null;
        case "game_stop":
          return fakeStop(String(a.portId));
        case "games_running":
          return [...playing].map(([port_id, started_at]) => ({
            port_id,
            state: { state: "running", started_at },
          })) satisfies GameChanged[];
        case "play_stats":
          return [...played.values()];
        case "plugin:opener|open_url":
          window.open(String(a.url), "_blank", "noopener");
          return null;
        default: {
          const social = socialCommand(cmd, a);
          if (social !== undefined) return social;
          const saved = savesCommand(cmd, a);
          if (saved !== undefined) return saved;
          const steam = steamCommand(cmd, a);
          if (steam !== undefined) return steam;
          throw { code: "E_API_STATUS", message: `Mock IPC has no command ${cmd}` };
        }
      }
    },
    { shouldMockEvents: true },
  );
}
